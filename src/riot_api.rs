use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder, Url};
use serde::Deserialize;
use time::{Duration, OffsetDateTime};
use tokio::time::{sleep, Duration as tDuration};

pub const RIOT_RATE_LIMIT_MS: tDuration = tDuration::from_millis(1200);

pub enum Game {
    LoL,
    TFT,
}

pub enum ServerIdentifier {
    EUNE,
    EUW,
}
impl ServerIdentifier {
    pub fn parse(s: &str) -> Result<ServerIdentifier, &str> {
        match s {
            "EUNE" => Ok(ServerIdentifier::EUNE),
            "EUW" => Ok(ServerIdentifier::EUW),
            _ => Err("Invalid server"),
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            ServerIdentifier::EUNE => "eun1",
            ServerIdentifier::EUW => "euw1",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SummonerDTO {
    // accountId: String,
    // profileIconId: i32,
    // revisionDate: i64,
    // name: String,
    // id: String,
    pub puuid: String,
    // summonerLevel: i64,
}
#[derive(Debug, Deserialize)]
pub struct MatchDTO {
    pub info: InfoDTO,
}
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct InfoDTO {
    pub gameDuration: i64,
}
#[derive(Debug, Deserialize)]
pub struct TFTMatchDTO {
    pub info: TFTInfoDTO,
}
#[derive(Debug, Deserialize)]
pub struct TFTInfoDTO {
    // game_length: f32,
    pub participants: Vec<TFTParticipantDTO>,
}
#[derive(Debug, Deserialize)]
pub struct TFTParticipantDTO {
    pub puuid: String,
    pub time_eliminated: f32,
}

pub type MatchesList = Vec<String>;

pub struct RiotAPIClient {
    client_lol: Client,
    client_tft: Client,
}

impl RiotAPIClient {
    pub async fn new(
        riot_token_lol: &str,
        riot_token_tft: &str,
    ) -> Result<RiotAPIClient, reqwest::Error> {
        let mut headers_lol = HeaderMap::new();
        headers_lol.insert(
            "X-Riot-Token",
            HeaderValue::from_str(riot_token_lol).unwrap(),
        );
        let mut headers_tft = HeaderMap::new();
        headers_tft.insert(
            "X-Riot-Token",
            HeaderValue::from_str(riot_token_tft).unwrap(),
        );
        Ok(RiotAPIClient {
            client_lol: ClientBuilder::new()
                .default_headers(headers_lol)
                .connection_verbose(true)
                .build()?,
            client_tft: ClientBuilder::new()
                .default_headers(headers_tft)
                .connection_verbose(true)
                .build()?,
        })
    }

    pub async fn get_summoner(
        &self,
        game: Game,
        server: &ServerIdentifier,
        summoner_name: &str,
    ) -> Result<SummonerDTO, reqwest::Error> {
        Ok(tokio::join!(
            sleep(RIOT_RATE_LIMIT_MS),
            match game {
                Game::LoL => self.client_lol.get(
                    Url::parse(&format!(
                        "https://{}.api.riotgames.com/lol/summoner/v4/summoners/by-name/{}",
                        server.as_str(),
                        summoner_name,
                    ))
                    .unwrap()
                ),
                Game::TFT => self.client_tft.get(
                    Url::parse(&format!(
                        "https://{}.api.riotgames.com/tft/summoner/v1/summoners/by-name/{}",
                        server.as_str(),
                        summoner_name,
                    ))
                    .unwrap()
                ),
            }
            .send()
            .await?
            .json::<SummonerDTO>()
        )
        .1?)
    }

    async fn playtime(&self, game: Game, puuid: &str) -> Result<(usize, i64), reqwest::Error> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let then = now.checked_sub(Duration::WEEK.whole_seconds()).unwrap();
        let mut secs = 0;
        let matches = tokio::join!(
            sleep(RIOT_RATE_LIMIT_MS),
            match game {
                Game::LoL => self.client_lol.get(format!(
                    "https://europe.api.riotgames.com/lol/match/v5/matches/by-puuid/{}/ids?startTime={}&endTime={}&start=0&count=100",
                    puuid, then, now
                )),
                Game::TFT => self.client_tft.get(format!(
                    "https://europe.api.riotgames.com/tft/match/v1/matches/by-puuid/{}/ids?startTime={}&endTime={}&start=0&count=100",
                    puuid, then, now
                ))
            }
            .send()
            .await?
            .json::<MatchesList>()
        ).1?;
        for m_id in &matches {
            match game {
                Game::LoL => {
                    let mtch = tokio::join!(
                        sleep(RIOT_RATE_LIMIT_MS),
                        self.client_lol
                            .get(format!(
                                "https://europe.api.riotgames.com/lol/match/v5/matches/{m_id}"
                            ))
                            .send()
                            .await?
                            .json::<MatchDTO>()
                    )
                    .1?;
                    secs += mtch.info.gameDuration;
                }
                Game::TFT => {
                    let mtch = tokio::join!(
                        sleep(RIOT_RATE_LIMIT_MS),
                        self.client_tft
                            .get(format!(
                                "https://europe.api.riotgames.com/tft/match/v1/matches/{m_id}"
                            ))
                            .send()
                            .await?
                            .json::<TFTMatchDTO>()
                    )
                    .1?;
                    secs += mtch
                        .info
                        .participants
                        .iter()
                        .find(|p| p.puuid == puuid)
                        .unwrap()
                        .time_eliminated as i64;
                }
            };
        }
        Ok((matches.len(), secs))
    }

    pub async fn get_playtime(
        &self,
        puuid_lol: &str,
        puuid_tft: &str,
    ) -> Result<(usize, i64), reqwest::Error> {
        let (ml, sl) = self.playtime(Game::LoL, puuid_lol).await?;
        let (mt, st) = self.playtime(Game::TFT, puuid_tft).await?;
        Ok((ml + mt, sl + st))
    }
}
