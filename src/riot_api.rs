use riven::{
    consts::{PlatformRoute, RegionalRoute},
    models::{summoner_v4, tft_summoner_v1},
    RiotApi, RiotApiError,
};
use time::{Duration, OffsetDateTime};

pub struct RiotAPIClient {
    client_lol: RiotApi,
    client_tft: RiotApi,
}

impl RiotAPIClient {
    pub fn new(riot_token_lol: &str, riot_token_tft: &str) -> Self {
        Self {
            client_lol: RiotApi::new(riot_token_lol),
            client_tft: RiotApi::new(riot_token_tft),
        }
    }

    pub async fn get_summoner_lol(
        &self,
        server: PlatformRoute,
        summoner_name: &str,
    ) -> Result<Option<summoner_v4::Summoner>, RiotApiError> {
        self.client_lol
            .summoner_v4()
            .get_by_summoner_name(server, summoner_name)
            .await
    }
    pub async fn get_summoner_tft(
        &self,
        server: PlatformRoute,
        summoner_name: &str,
    ) -> Result<Option<tft_summoner_v1::Summoner>, RiotApiError> {
        self.client_tft
            .tft_summoner_v1()
            .get_by_summoner_name(server, summoner_name)
            .await
    }

    async fn weekly_playtime_lol(
        &self,
        region: RegionalRoute,
        puuid: &str,
    ) -> Result<(usize, i64), RiotApiError> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let then = now.checked_sub(Duration::WEEK.whole_seconds()).unwrap();
        let mut secs = 0;
        let matches = self
            .client_lol
            .match_v5()
            .get_match_ids_by_puuid(region, puuid, Some(100), None, None, Some(then), None, None)
            .await?;
        for m_id in &matches {
            let mtch = self
                .client_lol
                .match_v5()
                .get_match(region, m_id)
                .await?
                .expect("Match not found");
            secs += mtch.info.game_duration;
        }
        Ok((matches.len(), secs))
    }
    async fn weekly_playtime_tft(
        &self,
        region: RegionalRoute,
        puuid: &str,
    ) -> Result<(usize, i64), RiotApiError> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let then = now.checked_sub(Duration::WEEK.whole_seconds()).unwrap();
        let mut secs = 0;
        let matches = self
            .client_tft
            .tft_match_v1()
            .get_match_ids_by_puuid(region, puuid, Some(100), None, None, Some(then))
            .await?;
        for m_id in &matches {
            let mtch = self
                .client_tft
                .tft_match_v1()
                .get_match(region, m_id)
                .await?
                .expect("Match not found");
            secs += mtch
                .info
                .participants
                .iter()
                .find(|p| p.puuid == puuid)
                .expect("Summoner not found")
                .time_eliminated as i64;
        }
        Ok((matches.len(), secs))
    }

    pub async fn get_playtime(
        &self,
        region: RegionalRoute,
        puuid_lol: &str,
        puuid_tft: &str,
    ) -> Result<(usize, i64), RiotApiError> {
        match tokio::join!(
            self.weekly_playtime_lol(region, puuid_lol),
            self.weekly_playtime_tft(region, puuid_tft),
        ) {
            (Ok((ml, sl)), Ok((mt, st))) => Ok((ml + mt, sl + st)),
            (Err(e), _) => Err(e),
            (Ok(_), Err(e)) => Err(e),
        }
    }
}
