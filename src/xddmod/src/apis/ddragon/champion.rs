use std::collections::HashMap;

use fake::Dummy;
use serde::Deserialize;
use serde::Serialize;
use sqlx::types::Json;
use sqlx::SqliteExecutor;

use crate::apis::ddragon::ChampionKey;

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
    pub async fn insert(&self, executor: impl SqliteExecutor<'_>) -> sqlx::Result<()> {
        let info = Json(&self.info);
        let image = Json(&self.image);
        let tags = Json(&self.tags);
        let stats = Json(&self.stats);

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

    pub async fn by_key(key: ChampionKey, executor: impl SqliteExecutor<'_>) -> sqlx::Result<Option<Champion>> {
        Ok(sqlx::query!(
            r#"
                select
                    version,
                    id,
                    key as "key!: ChampionKey",
                    name,
                    title,
                    blurb,
                    info as "info!: Json<Info>",
                    image as "image!: Json<Image>", 
                    tags as "tags!: Json<Vec<Tag>>", 
                    partype, 
                    stats as "stats!: Json<HashMap<String, f64>>" 
                from champions 
                where key = $1
            "#,
            key as _
        )
        .fetch_optional(executor)
        .await?
        .map(|r| Self {
            version: r.version,
            id: r.id,
            key: r.key,
            name: r.name,
            title: r.title,
            blurb: r.blurb,
            info: r.info.0,
            image: r.image.0,
            tags: r.tags.0,
            partype: r.partype,
            stats: r.stats.0,
        }))
    }

    pub async fn truncate(executor: impl SqliteExecutor<'_>) -> sqlx::Result<()> {
        sqlx::query!(r#"delete from champions"#).execute(executor).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Dummy)]
pub struct Image {
    pub full: String,
    pub sprite: Sprite,
    pub group: Type,
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
pub enum Type {
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
