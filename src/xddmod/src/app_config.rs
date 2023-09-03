use std::net::SocketAddr;

use config::Config;
use config::Environment;
use serde::Deserialize;
use twitch_api::twitch_oauth2::ClientId;
use twitch_api::twitch_oauth2::ClientSecret;
use url::Url;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub socket_addr: SocketAddr,
    pub server_url: Url,
    pub db_url: Url,
    pub client_id: ClientId,
    pub client_secret: ClientSecret,
}

impl AppConfig {
    pub fn init() -> Self {
        Config::builder()
            .add_source(Environment::default())
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap()
    }
}
