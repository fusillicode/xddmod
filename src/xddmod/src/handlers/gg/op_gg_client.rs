use std::fmt::Display;

use anyhow::bail;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use fake::Dummy;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DurationSeconds;

const OP_GG_API: &str = "https://op.gg/api/v1.0/internal/bypass";

#[derive(Clone, Copy, Debug, Dummy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    Br,
    Eune,
    Euw,
    Jp,
    Kr,
    Lan,
    Las,
    Na,
    Oce,
    Ph,
    Ru,
    Sg,
    Th,
    Tr,
    Tw,
    Vn,
}

impl Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Region::Br => "br",
            Region::Eune => "eune",
            Region::Euw => "euw",
            Region::Jp => "jp",
            Region::Kr => "kr",
            Region::Lan => "lan",
            Region::Las => "las",
            Region::Na => "na",
            Region::Oce => "oce",
            Region::Ph => "ph",
            Region::Ru => "ru",
            Region::Sg => "sg",
            Region::Th => "th",
            Region::Tr => "tr",
            Region::Tw => "tw",
            Region::Vn => "vn",
        };
        write!(f, "{}", s)
    }
}

pub async fn get_summoner(region: Region, summoner_name: &str) -> anyhow::Result<Summoner> {
    match &get_summoners(region, summoner_name).await?.data[..] {
        [summoner] => Ok(summoner.clone()),
        [] => bail!("No summoner found with name {} in region {}", summoner_name, region),
        summoners => bail!(
            "Multiple summoners found with name {} in region {}, summoners: {:?}",
            summoner_name,
            region,
            summoners
        ),
    }
}

pub async fn get_games(
    region: Region,
    summoner_id: &str,
    maybe_from: Option<DateTime<Utc>>,
    maybe_to: Option<DateTime<Utc>>,
) -> anyhow::Result<Games> {
    let mut url = Url::parse(&format!("{}/games/{}/summoners/{}", OP_GG_API, region, summoner_id))?;

    fn build_query(ended_at: DateTime<Utc>) -> String {
        format!("game_type=total&ended_at={}", ended_at.to_rfc3339())
    }

    if let Some(to) = maybe_to {
        url.set_query(Some(&build_query(to)));
    }

    let mut games: Games = reqwest::get(url.clone()).await?.json().await?;

    if let Some(from) = maybe_from {
        while games.meta.first_game_created_at > from {
            url.set_query(Some(&build_query(games.meta.last_game_created_at)));

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

async fn get_summoners(region: Region, summoner_name: &str) -> anyhow::Result<Summoners> {
    let mut url = Url::parse(&format!("{}/summoners/{}/autocomplete", OP_GG_API, region))?;
    url.set_query(Some(&format!("keyword={}", summoner_name)));

    Ok(reqwest::get(url).await?.json().await?)
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Summoners {
    pub data: Vec<Summoner>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Summoner {
    pub id: i64,
    pub summoner_id: String,
    pub acct_id: String,
    pub puuid: String,
    pub name: String,
    pub internal_name: String,
    pub profile_image_url: String,
    pub level: i64,
    pub updated_at: DateTime<Utc>,
    pub solo_tier_info: Option<TierInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Games {
    pub data: Vec<Game>,
    pub meta: Meta,
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Game {
    pub id: String,
    pub memo: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    #[serde(alias = "game_map")]
    pub map: String,
    pub queue_info: QueueInfo,
    pub version: String,
    #[serde(alias = "game_length_second")]
    #[serde_as(as = "DurationSeconds<i64>")]
    pub duration: Duration,
    pub is_remake: bool,
    pub is_opscore_active: bool,
    pub is_recorded: bool,
    pub average_tier_info: TierInfo,
    pub participants: Vec<Participant>,
    pub teams: Vec<Team>,
    #[serde(alias = "myData")]
    pub my_data: Participant,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct TierInfo {
    pub tier: Option<String>,
    pub division: Option<i64>,
    pub tier_image_url: String,
    pub border_image_url: Option<String>,
    pub lp: Option<i64>,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Dummy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeamKey {
    Red,
    Blue,
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
