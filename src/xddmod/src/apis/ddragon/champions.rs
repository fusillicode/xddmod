use std::collections::HashMap;

use fake::Dummy;
use serde::Deserialize;
use serde::Serialize;
use sqlx::types::Json;
use sqlx::SqliteExecutor;

use crate::apis::ddragon::ChampionKey;

pub async fn get_champion(champion_key: impl Into<ChampionKey>) -> anyhow::Result<Option<Champion>> {
    Ok(get_champions().await?.get(&champion_key.into()).cloned())
}

pub async fn get_champions() -> anyhow::Result<HashMap<ChampionKey, Champion>> {
    // FIXME: pls 🥲
    let api_response: ApiResponse = serde_json::from_str(&std::fs::read_to_string("./champion.json")?)?;
    Ok(api_response.data.into_values().map(|c| (c.key.clone(), c)).collect())
}

// FIXME: remove this after xtask import is ready
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
    pub key: ChampionKey,
    pub name: String,
    pub title: String,
    pub blurb: String,
    pub info: Info,
    pub image: Image,
    pub tags: Vec<Tag>,
    pub partype: String,
    pub stats: HashMap<String, f64>,
}

impl Champion {
    pub async fn insert(self, executor: impl SqliteExecutor<'_>) -> sqlx::Result<()> {
        let info = Json(self.info);
        let image = Json(self.image);
        let tags = Json(self.tags);
        let stats = Json(self.stats);

        sqlx::query!(
            r#"
                insert into champions (
                    version,
                    id,
                    key,
                    name,
                    title,
                    blurb,
                    info,
                    image,
                    tags,
                    partype,
                    stats
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            self.version,
            self.id,
            self.key as _,
            self.name as _,
            self.title as _,
            self.blurb as _,
            info as _,
            image as _,
            tags as _,
            self.partype as _,
            stats as _,
        )
        .execute(executor)
        .await
        .map(|_| ())
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, Dummy, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum Kind {
    #[serde(alias = "champion")]
    Champion,
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
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

#[derive(Debug, Clone, Serialize, Deserialize, Dummy, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum Tag {
    Assassin,
    Fighter,
    Mage,
    Marksman,
    Support,
    Tank,
}
