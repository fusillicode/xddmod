use reqwest::Url;
use xddmod::apis::ddragon::champions::ApiResponse;

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
