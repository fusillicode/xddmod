use std::collections::HashMap;

use fake::Dummy;
use fake::Fake;
use fake::Faker;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Champion {
    pub version: String,
    pub id: String,
    pub key: ChampionKey,
    pub name: String,
    pub title: String,
    pub blurb: String,
    pub info: Json<Info>,
    pub image: Json<Image>,
    pub tags: Json<Vec<Tag>>,
    pub partype: String,
    pub stats: Json<HashMap<String, f64>>,
}

impl Champion {
    pub async fn insert(&self, executor: impl SqliteExecutor<'_>) -> sqlx::Result<()> {
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
            self.info as _,
            self.image as _,
            self.tags as _,
            self.partype as _,
            self.stats as _,
        )
        .execute(executor)
        .await
        .map(|_| ())
    }

    pub async fn by_key(key: ChampionKey, executor: impl SqliteExecutor<'_>) -> sqlx::Result<Option<Champion>> {
        sqlx::query_as!(
            Self,
            r#"
                select
                    version as "version!: _",
                    id as "id!: _",
                    key as "key!: ChampionKey",
                    name as "name!: _",
                    title as "title!: _",
                    blurb as "blurb!: _",
                    info as "info!: Json<Info>",
                    image as "image!: Json<Image>", 
                    tags as "tags!: Json<Vec<Tag>>", 
                    partype as "partype!: _", 
                    stats as "stats!: Json<HashMap<String, f64>>" 
                from champions 
                where key = $1
            "#,
            key as _
        )
        .fetch_optional(executor)
        .await
    }
}

impl Dummy<Faker> for Champion {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_config: &Faker, rng: &mut R) -> Self {
        Self {
            version: Faker.fake_with_rng(rng),
            id: Faker.fake_with_rng(rng),
            key: Faker.fake_with_rng(rng),
            name: Faker.fake_with_rng(rng),
            title: Faker.fake_with_rng(rng),
            blurb: Faker.fake_with_rng(rng),
            info: Json(Faker.fake_with_rng(rng)),
            image: Json(Faker.fake_with_rng(rng)),
            tags: Json(Faker.fake_with_rng(rng)),
            partype: Faker.fake_with_rng(rng),
            stats: Json(Faker.fake_with_rng(rng)),
        }
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
