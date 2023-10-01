use std::collections::HashMap;

use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use xddmod::apis::ddragon::champion::Champion;
use xddmod::apis::ddragon::champion::Type;

#[derive(clap::Args)]
pub struct ImportDdragonChampion {
    /// Base url of ddragon API
    #[arg(long)]
    ddragon_api_base_url: Url,
    /// DB Url
    #[arg(long)]
    db_url: Url,
}

impl ImportDdragonChampion {
    pub async fn run(self) -> anyhow::Result<()> {
        let db_pool = SqlitePool::connect(self.db_url.as_ref()).await?;

        let api_response: ApiResponse = reqwest::get(format!("{}/champion.json", self.ddragon_api_base_url))
            .await?
            .json()
            .await?;

        let mut tx = db_pool.begin().await?;
        Champion::truncate(&mut tx).await?;
        for champion in api_response.data.into_values() {
            champion.insert(&mut tx).await?;
        }
        tx.commit().await?;

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
