use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use itertools::Itertools;
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

    pub async fn tft_analysis(
        &self,
        region: RegionalRoute,
        puuid: &str,
    ) -> Result<Vec<String>, RiotApiError> {
        let matches = self
            .client_tft
            .tft_match_v1()
            .get_match_ids_by_puuid(region, puuid, Some(90), None, None, None)
            .await?;
        let mut res = Vec::new();
        let mut trait_ranking = WinrateList::new();
        let mut unit_ranking = WinrateList::new();
        let mut item_ranking = WinrateList::new();
        let mut augment_ranking = WinrateList::new();
        let mut set = "<unknown>".to_owned();
        let mut matches_analyzed = 0;
        for m_id in &matches {
            let mtch = self
                .client_tft
                .tft_match_v1()
                .get_match(region, m_id)
                .await?
                .expect("Match not found");
            if set == "<unknown>" {
                set = mtch.info.tft_set_core_name.to_owned();
            }
            if mtch.info.tft_set_core_name != set {
                break;
            }
            let player = mtch
                .info
                .participants
                .iter()
                .find(|p| p.puuid == puuid)
                .expect("Summoner not found");
            let placement = if mtch.info.tft_game_type == "pairs" {
                match player.placement {
                    1 => 1.0,
                    2 => 1.0,
                    3 => 1.0 + 7.0 / 3.0,
                    4 => 1.0 + 7.0 / 3.0,
                    5 => 1.0 + 14.0 / 3.0,
                    6 => 1.0 + 14.0 / 3.0,
                    7 => 8.0,
                    8 => 8.0,
                    _ => 1000.0,
                }
            } else {
                player.placement as f32
            };
            let mut traits = player.traits.clone();
            traits.sort_by_key(|t| -t.style.unwrap_or(0));
            for t in traits {
                if t.style.unwrap_or(0) < 2 {
                    break;
                }
                trait_ranking.insert(t.name, &placement);
            }
            let mut units = HashSet::new();
            for uid in player
                .units
                .clone()
                .iter()
                .map(|u| u.character_id.to_owned())
            {
                units.insert(uid);
            }
            for u in units {
                unit_ranking.insert(u, &placement);
            }
            let mut items = HashSet::new();
            for i in player
                .units
                .clone()
                .iter()
                .flat_map(|u| u.item_names.to_owned())
            {
                items.insert(i);
            }
            for i in items {
                item_ranking.insert(i, &placement);
            }
            for a in player.augments.clone() {
                augment_ranking.insert(a, &placement);
            }
            matches_analyzed += 1;
        }
        let low_dataset_bound = (matches_analyzed as f32 * 0.07) as usize;
        res.push(format!(
            "{} matches from {} analyzed.\nHiding data with less than {} samples.",
            matches_analyzed, set, low_dataset_bound
        ));
        for (title, ranking) in vec![
            ("Traits (Silver+)", trait_ranking),
            ("Units", unit_ranking),
            ("Items", item_ranking),
            ("Augments", augment_ranking),
        ] {
            let mut s = String::new();
            s.push_str(&format!("{title}\n"));
            s.push_str("```\n");
            let mut line = false;
            for (t, (p, g)) in ranking
                .sorted_iter()
                .filter(|(_t, (_p, g))| g > &low_dataset_bound)
            {
                let avg = p / g as f32;
                if !line && avg >= 4.5 {
                    s.push_str("---\n");
                    line = true;
                }
                s.push_str(&format!("{: <15} {:.1} ({})\n", last_part(t), avg, g));
            }
            s.push_str("```");
            res.push(s);
        }
        Ok(res)
    }
}

fn last_part(s: String) -> String {
    s.split_terminator('_').last().unwrap_or("???").to_owned()
}

#[derive(Debug)]
struct WinrateList<T> {
    pub map: HashMap<T, (f32, usize)>,
}

impl<T> WinrateList<T>
where
    T: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    pub fn insert(&mut self, entry: T, placement: &f32) {
        let v = match self.map.get(&entry) {
            Some((placements, total)) => (placements + *placement, total + 1),
            None => (*placement, 1),
        };
        self.map.insert(entry, v);
    }
    pub fn sorted_iter(self) -> std::vec::IntoIter<(T, (f32, usize))> {
        self.map
            .into_iter()
            .sorted_by_key(|(_t, (p, g))| ((p / *g as f32) * 1000.0) as i32)
    }
}

impl<T> Clone for WinrateList<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}
