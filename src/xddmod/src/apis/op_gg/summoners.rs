use anyhow::bail;
use chrono::DateTime;
use chrono::Utc;
use fake::Dummy;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

use crate::apis::op_gg::Region;
use crate::apis::op_gg::TierInfo;
use crate::apis::op_gg::OP_GG_API;

pub async fn get_summoner(region: Region, summoner_name: &str) -> anyhow::Result<Summoner> {
    match &get_summoners(region, summoner_name).await?.data[..] {
        [summoner] => Ok(summoner.clone()),
        [] => bail!("No summoner found with name {} in region {}", summoner_name, region),
        summoners => bail!(
            "Multiple summoners found with name {} in region {}, summoners: {:?}",
            summoner_name,
            region,
            summoners
        ),
    }
}

async fn get_summoners(region: Region, summoner_name: &str) -> anyhow::Result<Summoners> {
    let mut url = Url::parse(&format!("{}/summoners/{}/autocomplete", OP_GG_API, region))?;
    url.set_query(Some(&format!("keyword={}", summoner_name)));

    Ok(reqwest::get(url).await?.json().await?)
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Summoners {
    pub data: Vec<Summoner>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Dummy)]
pub struct Summoner {
    pub id: i64,
    pub summoner_id: String,
    pub acct_id: String,
    pub puuid: String,
    pub name: String,
    pub internal_name: String,
    pub profile_image_url: String,
    pub level: i64,
    pub updated_at: DateTime<Utc>,
    pub solo_tier_info: Option<TierInfo>,
}
