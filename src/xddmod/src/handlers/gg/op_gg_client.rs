use anyhow::anyhow;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

const OP_GG_API_ENDPOINT: &str = "https://op.gg/api/v1.0/internal/bypass";

pub async fn get_summoner(region: &str, summoner_name: &str) -> anyhow::Result<Summoner> {
    let summoners = get_summoners(region, summoner_name).await?;
    match summoners {

    }
}

pub async fn get_summoners(region: &str, summoner_name: &str) -> anyhow::Result<Summoners> {
    let mut api = Url::parse(&format!("{}/{}/autocomplete", OP_GG_API_ENDPOINT, region))?;
    api.set_query(Some(summoner_name));

    Ok(reqwest::get(api).await?.json().await?)
}

#[derive(Serialize, Deserialize)]
pub struct Summoners {
    data: Vec<Summoner>,
}

#[derive(Serialize, Deserialize)]
pub struct Summoner {
    id: i64,
    summoner_id: String,
    acct_id: String,
    puuid: String,
    name: String,
    internal_name: String,
    profile_image_url: String,
    level: i64,
    updated_at: String,
    solo_tier_info: SoloTierInfo,
}

#[derive(Serialize, Deserialize)]
pub struct SoloTierInfo {
    tier: String,
    division: i64,
    lp: i64,
    tier_image_url: String,
    border_image_url: String,
}
