use std::collections::HashMap;

use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use sqlx::types::Json;
use sqlx::SqlitePool;
use xddmod::apis::ddragon::champions::Image;
use xddmod::apis::ddragon::champions::Info;
use xddmod::apis::ddragon::champions::Tag;
use xddmod::apis::ddragon::champions::Type;
use xddmod::apis::ddragon::ChampionKey;

#[derive(clap::Args)]
pub struct ImportDdragonChampions {
    /// Base url of ddragon API
    #[arg(long)]
    ddragon_api_base_url: Url,
    /// DB Url
    #[arg(long)]
    db_url: Url,
}

impl ImportDdragonChampions {
    pub async fn run(self) -> anyhow::Result<()> {
        let db_pool = SqlitePool::connect(self.db_url.as_ref()).await.unwrap();

        let api_response: ApiResponse = reqwest::get(format!("{}/champion.json", self.ddragon_api_base_url))
            .await?
            .json()
            .await?;

        let mut tx = db_pool.begin().await.unwrap();
        xddmod::apis::ddragon::champions::Champion::truncate(&mut tx)
            .await
            .unwrap();
        for champion in api_response.data.into_values() {
            xddmod::apis::ddragon::champions::Champion::from(champion)
                .insert(&mut tx)
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiResponse {
    pub r#type: Type,
    pub format: String,
    pub version: String,
    pub data: HashMap<String, Champion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Champion {
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

impl From<Champion> for xddmod::apis::ddragon::champions::Champion {
    fn from(val: Champion) -> Self {
        xddmod::apis::ddragon::champions::Champion {
            version: val.version,
            id: val.id,
            key: val.key,
            name: val.name,
            title: val.title,
            blurb: val.blurb,
            info: Json(val.info),
            image: Json(val.image),
            tags: Json(val.tags),
            partype: val.partype,
            stats: Json(val.stats),
        }
    }
}
