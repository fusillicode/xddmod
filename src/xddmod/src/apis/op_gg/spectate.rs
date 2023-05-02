use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use fake::Dummy;
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

#[derive(Serialize, Deserialize, Debug, Dummy)]
#[serde(untagged)]
pub enum SpectateStatus {
    NotInGame(NotInGame),
    InGame(InGame),
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct NotInGame {
    pub code: i64,
    pub message: String,
    pub detail: Detail,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct Detail {
    pub param: String,
    #[serde(alias = "detailMessage")]
    pub detail_message: String,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct InGame {
    pub data: Game,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct Game {
    pub game_id: String,
    pub created_at: DateTime<Utc>,
    #[serde(alias = "game_map")]
    pub map: String,
    pub queue_info: QueueInfo,
    pub record_status: String,
    pub participants: Vec<Participant>,
    pub teams: Vec<Team>,
    #[serde(alias = "championsById")]
    pub champions_by_key: HashMap<ChampionKey, Champion>,
    pub seasons: Vec<Season>,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct Champion {
    #[serde(rename(deserialize = "key"))]
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct Participant {
    pub summoner: Summoner,
    pub team_key: TeamKey,
    #[serde(alias = "champion_id", deserialize_with = "champion_key_from_i64")]
    pub champion_key: ChampionKey,
    pub position: String,
    pub rune_build: RuneBuild,
    pub spells: Vec<i64>,
    pub most_champion_stat: Option<HashMap<String, i64>>,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
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
    pub team_info: Option<serde_json::Value>,
    pub previous_seasons: Vec<PreviousSeason>,
    pub league_stats: Vec<LeagueStat>,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct RuneBuild {
    pub primary_page_id: i64,
    pub primary_rune_ids: Vec<i64>,
    pub secondary_page_id: i64,
    pub secondary_rune_ids: Vec<i64>,
    pub stat_mod_ids: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct LeagueStat {
    pub queue_info: QueueInfo,
    pub tier_info: TierInfo,
    pub win: i64,
    pub lose: i64,
    pub is_hot_streak: bool,
    pub is_fresh_blood: bool,
    pub is_veteran: bool,
    pub is_inactive: bool,
    pub series: Option<serde_json::Value>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct QueueInfo {
    pub id: i64,
    pub game_type: String,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct PreviousSeason {
    pub season_id: i64,
    pub tier_info: TierInfo,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct Season {
    pub id: i64,
    pub value: i64,
    pub display_value: i64,
    pub is_preseason: bool,
}

#[derive(Serialize, Deserialize, Debug, Dummy)]
pub struct Team {
    pub key: TeamKey,
    pub average_tier_info: TierInfo,
    pub banned_champions: Vec<Option<serde_json::Value>>,
}

fn champion_key_from_i64<'de, D>(deserializer: D) -> Result<ChampionKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(ChampionKey::from(i64::deserialize(deserializer)?))
}
