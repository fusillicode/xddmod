use std::collections::HashMap;

use fake::Dummy;
use serde::Deserialize;
use serde::Serialize;

use crate::apis::ddragon::DDRAGON_API;

pub async fn get_champion(champion_key: impl Into<ChampionKey>) -> anyhow::Result<Option<Champion>> {
    Ok(get_champions().await?.get(&champion_key.into()).cloned())
}

pub async fn get_champions() -> anyhow::Result<HashMap<ChampionKey, Champion>> {
    let api_response: ApiResponse = reqwest::get(format!("{}/champions.json", DDRAGON_API))
        .await?
        .json()
        .await?;

    Ok(api_response
        .data
        .into_values()
        .map(|c| (ChampionKey(c.key.clone()), c))
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dummy)]
pub struct ChampionKey(String);

impl From<String> for ChampionKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ChampionKey {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<i64> for ChampionKey {
    fn from(value: i64) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub struct ApiResponse {
    #[serde(alias = "type")]
    pub kind: Kind,
    pub format: String,
    pub version: String,
    pub data: HashMap<String, Champion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub struct Champion {
    pub version: String,
    pub id: String,
    pub key: String,
    pub name: String,
    pub title: String,
    pub blurb: String,
    pub info: Info,
    pub image: Image,
    pub tags: Vec<Tag>,
    pub partype: String,
    pub stats: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub struct Image {
    pub full: String,
    pub sprite: Sprite,
    pub group: Kind,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub struct Info {
    pub attack: i64,
    pub defense: i64,
    pub magic: i64,
    pub difficulty: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub enum Kind {
    #[serde(alias = "champion")]
    Champion,
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub enum Sprite {
    #[serde(alias = "champion0.png")]
    Champion0Png,
    #[serde(alias = "champion1.png")]
    Champion1Png,
    #[serde(alias = "champion2.png")]
    Champion2Png,
    #[serde(alias = "champion3.png")]
    Champion3Png,
    #[serde(alias = "champion4.png")]
    Champion4Png,
    #[serde(alias = "champion5.png")]
    Champion5Png,
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub enum Tag {
    Assassin,
    Fighter,
    Mage,
    Marksman,
    Support,
    Tank,
}
