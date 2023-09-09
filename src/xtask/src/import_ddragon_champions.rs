use std::collections::HashMap;

use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use xddmod::apis::ddragon::champions::Champion;
use xddmod::apis::ddragon::champions::Type;

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
        Champion::truncate(&mut tx).await.unwrap();
        for champion in api_response.data.into_values() {
            champion.insert(&mut tx).await.unwrap();
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
