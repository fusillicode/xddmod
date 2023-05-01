use chrono::DateTime;
use chrono::Utc;
use fake::Dummy;
use fake::Fake;
use fake::Faker;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

use crate::apis::op_gg::summoners::Summoner;
use crate::apis::op_gg::Region;
use crate::apis::op_gg::TeamKey;
use crate::apis::op_gg::TierInfo;
use crate::apis::op_gg::OP_GG_API;

pub async fn get_last_game(region: Region, summoner_id: &str) -> anyhow::Result<Option<Game>> {
    let games = get_games(region, summoner_id, None, None, Some(1)).await?;

    Ok(games.data.first().cloned())
}

async fn get_games(
    region: Region,
    summoner_id: &str,
    maybe_from: Option<DateTime<Utc>>,
    maybe_to: Option<DateTime<Utc>>,
    maybe_limit: Option<i32>,
) -> anyhow::Result<Games> {
    let mut url = Url::parse(&format!("{}/games/{}/summoners/{}", OP_GG_API, region, summoner_id))?;

    fn build_query(maybe_to: Option<DateTime<Utc>>, maybe_limit: Option<i32>) -> String {
        let mut query = "game_type=total".to_owned();
        if let Some(limit) = maybe_limit {
            query.push_str(&format!("&limit={}", limit))
        }
        if let Some(to) = maybe_to {
            query.push_str(&format!("&ended_at={}", to.to_rfc3339()))
        }
        query
    }

    url.set_query(Some(&build_query(maybe_to, maybe_limit)));

    let mut games: Games = reqwest::get(url.clone()).await?.json().await?;

    if let Some(from) = maybe_from {
        while games.meta.first_game_created_at > from {
            url.set_query(Some(&build_query(Some(games.meta.last_game_created_at), None)));

            let mut old_games: Games = reqwest::get(url.clone()).await?.json().await?;

            games.data.append(&mut old_games.data);
            games.meta = Meta {
                first_game_created_at: old_games.meta.last_game_created_at,
                ..games.meta
            };
        }
    }

    Ok(games)
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Games {
    pub data: Vec<Game>,
    pub meta: Meta,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Game {
    pub id: String,
    pub memo: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    #[serde(alias = "game_map")]
    pub map: String,
    pub queue_info: QueueInfo,
    pub version: String,
    #[serde(alias = "game_length_second", deserialize_with = "std_duration_from_u64")]
    pub duration: std::time::Duration,
    pub is_remake: bool,
    pub is_opscore_active: bool,
    pub is_recorded: bool,
    pub average_tier_info: TierInfo,
    pub participants: Vec<Participant>,
    pub teams: Vec<Team>,
    #[serde(alias = "myData")]
    pub my_data: Participant,
}

impl Dummy<Faker> for Game {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &Faker, rng: &mut R) -> Self {
        Self {
            id: Faker.fake_with_rng(rng),
            memo: Faker.fake_with_rng(rng),
            created_at: Faker.fake_with_rng(rng),
            map: Faker.fake_with_rng(rng),
            queue_info: Faker.fake_with_rng(rng),
            version: Faker.fake_with_rng(rng),
            duration: std::time::Duration::new(Faker.fake_with_rng(rng), Faker.fake_with_rng(rng)),
            is_remake: Faker.fake_with_rng(rng),
            is_opscore_active: Faker.fake_with_rng(rng),
            is_recorded: Faker.fake_with_rng(rng),
            average_tier_info: Faker.fake_with_rng(rng),
            participants: Faker.fake_with_rng(rng),
            teams: Faker.fake_with_rng(rng),
            my_data: Faker.fake_with_rng(rng),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Participant {
    pub summoner: Summoner,
    pub participant_id: i64,
    #[serde(alias = "champion_id")]
    pub champion_key: i64,
    pub team_key: TeamKey,
    pub position: String,
    pub items: Vec<i64>,
    pub trinket_item: i64,
    pub rune: Rune,
    pub spells: Vec<i64>,
    pub stats: Stats,
    pub tier_info: TierInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Rune {
    pub primary_page_id: i64,
    pub primary_rune_id: i64,
    pub secondary_page_id: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Stats {
    pub champion_level: i64,
    pub damage_self_mitigated: i64,
    pub damage_dealt_to_objectives: i64,
    pub damage_dealt_to_turrets: i64,
    pub magic_damage_dealt_player: i64,
    pub physical_damage_taken: i64,
    pub physical_damage_dealt_to_champions: i64,
    pub total_damage_taken: i64,
    pub total_damage_dealt: i64,
    pub total_damage_dealt_to_champions: i64,
    pub largest_critical_strike: i64,
    pub time_ccing_others: i64,
    pub vision_score: i64,
    pub vision_wards_bought_in_game: i64,
    pub sight_wards_bought_in_game: i64,
    pub ward_kill: i64,
    pub ward_place: i64,
    pub turret_kill: i64,
    pub barrack_kill: i64,
    pub kill: i64,
    pub death: i64,
    pub assist: i64,
    pub largest_multi_kill: i64,
    pub largest_killing_spree: i64,
    pub minion_kill: i64,
    pub neutral_minion_kill_team_jungle: Option<serde_json::Value>,
    pub neutral_minion_kill_enemy_jungle: Option<serde_json::Value>,
    pub neutral_minion_kill: i64,
    pub gold_earned: i64,
    pub total_heal: i64,
    pub result: String,
    pub op_score: f64,
    pub op_score_rank: i64,
    pub is_opscore_max_in_team: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct QueueInfo {
    pub id: i64,
    pub queue_translate: String,
    pub game_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Team {
    pub key: TeamKey,
    pub game_stat: GameStat,
    pub banned_champions: Vec<Option<i64>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct GameStat {
    pub dragon_kill: i64,
    pub baron_kill: i64,
    pub tower_kill: i64,
    pub is_remake: bool,
    pub is_win: bool,
    pub kill: i64,
    pub death: i64,
    pub assist: i64,
    pub gold_earned: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Meta {
    pub first_game_created_at: DateTime<Utc>,
    pub last_game_created_at: DateTime<Utc>,
}

fn std_duration_from_u64<'de, D>(deserializer: D) -> Result<std::time::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(std::time::Duration::from_secs(u64::deserialize(deserializer)?))
}
