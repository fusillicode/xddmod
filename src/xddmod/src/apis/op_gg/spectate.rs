use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

use crate::apis::ddragon::ChampionKey;
use crate::apis::op_gg::Region;
use crate::apis::op_gg::TeamKey;
use crate::apis::op_gg::TierInfo;
use crate::apis::op_gg::OP_GG_API;

pub async fn get_spectate_status(region: Region, summoner_id: &str) -> anyhow::Result<SpectateStatus> {
    let url = Url::parse(&format!("{}/spectates/{}/{}", OP_GG_API, region, summoner_id))?;
    let spectate_status = reqwest::get(url.clone()).await?.json().await?;

    Ok(spectate_status)
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum SpectateStatus {
    NotInGame(NotInGame),
    InGame(InGame),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NotInGame {
    code: i64,
    message: String,
    detail: Detail,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Detail {
    param: String,
    #[serde(alias = "detailMessage")]
    detail_message: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InGame {
    data: Game,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Game {
    game_id: String,
    created_at: DateTime<Utc>,
    #[serde(alias = "game_map")]
    map: String,
    queue_info: QueueInfo,
    record_status: String,
    participants: Vec<Participant>,
    teams: Vec<Team>,
    #[serde(alias = "championsById")]
    champions_by_key: HashMap<ChampionKey, Champion>,
    seasons: Vec<Season>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Champion {
    #[serde(rename = "key")]
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Participant {
    summoner: Summoner,
    team_key: TeamKey,
    #[serde(alias = "champion_id")]
    champion_key: i64,
    position: String,
    rune_build: RuneBuild,
    spells: Vec<i64>,
    most_champion_stat: Option<HashMap<String, i64>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Summoner {
    id: i64,
    summoner_id: String,
    acct_id: String,
    puuid: String,
    name: String,
    internal_name: String,
    profile_image_url: String,
    level: i64,
    updated_at: DateTime<Utc>,
    team_info: Option<serde_json::Value>,
    previous_seasons: Vec<PreviousSeason>,
    league_stats: Vec<LeagueStat>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RuneBuild {
    primary_page_id: i64,
    primary_rune_ids: Vec<i64>,
    secondary_page_id: i64,
    secondary_rune_ids: Vec<i64>,
    stat_mod_ids: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeagueStat {
    queue_info: QueueInfo,
    tier_info: TierInfo,
    win: i64,
    lose: i64,
    is_hot_streak: bool,
    is_fresh_blood: bool,
    is_veteran: bool,
    is_inactive: bool,
    series: Option<serde_json::Value>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct QueueInfo {
    id: i64,
    game_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreviousSeason {
    season_id: i64,
    tier_info: TierInfo,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Season {
    id: i64,
    value: i64,
    display_value: i64,
    is_preseason: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Team {
    key: TeamKey,
    average_tier_info: TierInfo,
    banned_champions: Vec<Option<serde_json::Value>>,
}
