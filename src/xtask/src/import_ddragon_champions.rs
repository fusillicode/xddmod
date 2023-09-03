use std::collections::HashMap;

use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use sqlx::types::Json;
use xddmod::apis::ddragon::champions::Image;
use xddmod::apis::ddragon::champions::Info;
use xddmod::apis::ddragon::champions::Kind;
use xddmod::apis::ddragon::champions::Tag;
use xddmod::apis::ddragon::ChampionKey;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
