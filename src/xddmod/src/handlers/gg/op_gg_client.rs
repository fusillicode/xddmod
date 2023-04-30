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
    #[serde(rename = "id")]
    pub id: i64,

    #[serde(rename = "summoner_id")]
    pub summoner_id: String,

    #[serde(rename = "acct_id")]
    pub acct_id: String,

    #[serde(rename = "puuid")]
    pub puuid: String,

    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "internal_name")]
    pub internal_name: String,

    #[serde(rename = "profile_image_url")]
    pub profile_image_url: String,

    #[serde(rename = "level")]
    pub level: i64,

    #[serde(rename = "updated_at")]
    pub updated_at: DateTime<Utc>,

    #[serde(rename = "solo_tier_info")]
    pub solo_tier_info: Option<TierInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Games {
    #[serde(rename = "data")]
    pub data: Vec<Game>,

    #[serde(rename = "meta")]
    pub meta: Meta,
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Game {
    #[serde(rename = "id")]
    pub id: String,

    #[serde(rename = "memo")]
    pub memo: Option<serde_json::Value>,

    #[serde(rename = "created_at")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "game_map")]
    pub map: String,

    #[serde(rename = "queue_info")]
    pub queue_info: QueueInfo,

    #[serde(rename = "version")]
    pub version: String,

    #[serde(rename = "game_length_second")]
    #[serde_as(as = "DurationSeconds<i64>")]
    pub duration: Duration,

    #[serde(rename = "is_remake")]
    pub is_remake: bool,

    #[serde(rename = "is_opscore_active")]
    pub is_opscore_active: bool,

    #[serde(rename = "is_recorded")]
    pub is_recorded: bool,

    #[serde(rename = "average_tier_info")]
    pub average_tier_info: TierInfo,

    #[serde(rename = "participants")]
    pub participants: Vec<Participant>,

    #[serde(rename = "teams")]
    pub teams: Vec<Team>,

    #[serde(rename(serialize = "my_data", deserialize = "myData"))]
    pub my_data: Participant,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct TierInfo {
    #[serde(rename = "tier")]
    pub tier: Option<String>,

    #[serde(rename = "division")]
    pub division: Option<i64>,

    #[serde(rename = "tier_image_url")]
    pub tier_image_url: String,

    #[serde(rename = "border_image_url")]
    pub border_image_url: Option<String>,

    #[serde(rename = "lp")]
    pub lp: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Participant {
    #[serde(rename = "summoner")]
    pub summoner: Summoner,

    #[serde(rename = "participant_id")]
    pub participant_id: i64,

    #[serde(rename = "champion_id")]
    pub champion_key: i64,

    #[serde(rename = "team_key")]
    pub team_key: TeamKey,

    #[serde(rename = "position")]
    pub position: String,

    #[serde(rename = "items")]
    pub items: Vec<i64>,

    #[serde(rename = "trinket_item")]
    pub trinket_item: i64,

    #[serde(rename = "rune")]
    pub rune: Rune,

    #[serde(rename = "spells")]
    pub spells: Vec<i64>,

    #[serde(rename = "stats")]
    pub stats: Stats,

    #[serde(rename = "tier_info")]
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
    #[serde(rename = "primary_page_id")]
    pub primary_page_id: i64,

    #[serde(rename = "primary_rune_id")]
    pub primary_rune_id: i64,

    #[serde(rename = "secondary_page_id")]
    pub secondary_page_id: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Stats {
    #[serde(rename = "champion_level")]
    pub champion_level: i64,

    #[serde(rename = "damage_self_mitigated")]
    pub damage_self_mitigated: i64,

    #[serde(rename = "damage_dealt_to_objectives")]
    pub damage_dealt_to_objectives: i64,

    #[serde(rename = "damage_dealt_to_turrets")]
    pub damage_dealt_to_turrets: i64,

    #[serde(rename = "magic_damage_dealt_player")]
    pub magic_damage_dealt_player: i64,

    #[serde(rename = "physical_damage_taken")]
    pub physical_damage_taken: i64,

    #[serde(rename = "physical_damage_dealt_to_champions")]
    pub physical_damage_dealt_to_champions: i64,

    #[serde(rename = "total_damage_taken")]
    pub total_damage_taken: i64,

    #[serde(rename = "total_damage_dealt")]
    pub total_damage_dealt: i64,

    #[serde(rename = "total_damage_dealt_to_champions")]
    pub total_damage_dealt_to_champions: i64,

    #[serde(rename = "largest_critical_strike")]
    pub largest_critical_strike: i64,

    #[serde(rename = "time_ccing_others")]
    pub time_ccing_others: i64,

    #[serde(rename = "vision_score")]
    pub vision_score: i64,

    #[serde(rename = "vision_wards_bought_in_game")]
    pub vision_wards_bought_in_game: i64,

    #[serde(rename = "sight_wards_bought_in_game")]
    pub sight_wards_bought_in_game: i64,

    #[serde(rename = "ward_kill")]
    pub ward_kill: i64,

    #[serde(rename = "ward_place")]
    pub ward_place: i64,

    #[serde(rename = "turret_kill")]
    pub turret_kill: i64,

    #[serde(rename = "barrack_kill")]
    pub barrack_kill: i64,

    #[serde(rename = "kill")]
    pub kill: i64,

    #[serde(rename = "death")]
    pub death: i64,

    #[serde(rename = "assist")]
    pub assist: i64,

    #[serde(rename = "largest_multi_kill")]
    pub largest_multi_kill: i64,

    #[serde(rename = "largest_killing_spree")]
    pub largest_killing_spree: i64,

    #[serde(rename = "minion_kill")]
    pub minion_kill: i64,

    #[serde(rename = "neutral_minion_kill_team_jungle")]
    pub neutral_minion_kill_team_jungle: Option<serde_json::Value>,

    #[serde(rename = "neutral_minion_kill_enemy_jungle")]
    pub neutral_minion_kill_enemy_jungle: Option<serde_json::Value>,

    #[serde(rename = "neutral_minion_kill")]
    pub neutral_minion_kill: i64,

    #[serde(rename = "gold_earned")]
    pub gold_earned: i64,

    #[serde(rename = "total_heal")]
    pub total_heal: i64,

    #[serde(rename = "result")]
    pub result: String,

    #[serde(rename = "op_score")]
    pub op_score: f64,

    #[serde(rename = "op_score_rank")]
    pub op_score_rank: i64,

    #[serde(rename = "is_opscore_max_in_team")]
    pub is_opscore_max_in_team: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct QueueInfo {
    #[serde(rename = "id")]
    pub id: i64,

    #[serde(rename = "queue_translate")]
    pub queue_translate: String,

    #[serde(rename = "game_type")]
    pub game_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Team {
    #[serde(rename = "key")]
    pub key: TeamKey,

    #[serde(rename = "game_stat")]
    pub game_stat: GameStat,

    #[serde(rename = "banned_champions")]
    pub banned_champions: Vec<Option<i64>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct GameStat {
    #[serde(rename = "dragon_kill")]
    pub dragon_kill: i64,

    #[serde(rename = "baron_kill")]
    pub baron_kill: i64,

    #[serde(rename = "tower_kill")]
    pub tower_kill: i64,

    #[serde(rename = "is_remake")]
    pub is_remake: bool,

    #[serde(rename = "is_win")]
    pub is_win: bool,

    #[serde(rename = "kill")]
    pub kill: i64,

    #[serde(rename = "death")]
    pub death: i64,

    #[serde(rename = "assist")]
    pub assist: i64,

    #[serde(rename = "gold_earned")]
    pub gold_earned: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Meta {
    #[serde(rename = "first_game_created_at")]
    pub first_game_created_at: DateTime<Utc>,

    #[serde(rename = "last_game_created_at")]
    pub last_game_created_at: DateTime<Utc>,
}
