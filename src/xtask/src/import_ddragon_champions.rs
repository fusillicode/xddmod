use std::collections::HashMap;

use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use xddmod::apis::ddragon::champions::Champion;
use xddmod::apis::ddragon::champions::Kind;

#[derive(clap::Args)]
pub struct ImportDdragonChampions {
    /// Base url of ddragon API
    #[arg(long)]
    ddragon_api_url: Url,
    /// DB Url
    #[arg(long)]
    db_url: Url,
}

impl ImportDdragonChampions {
    pub async fn run(self) -> anyhow::Result<()> {
        let _api_response: ApiResponse = reqwest::get(format!("{}/champion.json", self.ddragon_api_url))
            .await?
            .json()
            .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    #[serde(alias = "type")]
    pub kind: Kind,
    pub format: String,
    pub version: String,
    pub data: HashMap<String, Champion>,
}
