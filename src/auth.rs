use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::async_trait;
use axum::extract::Query;
use axum::extract::State;
use axum::routing::get;
use axum::Router;
use config::Config;
use config::Environment;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Mutex;
use twitch_irc::login::GetAccessTokenResponse;
use twitch_irc::login::RefreshingLoginCredentials;
use twitch_irc::login::TokenStorage;
use twitch_irc::login::UserAccessToken;
use twitch_irc::message::ServerMessage;
use twitch_irc::TwitchIRCClient;
use twitch_oauth2::tokens::UserTokenBuilder;
use twitch_oauth2::TwitchToken;
use url::Url;

pub async fn authenticate(
    app_config: AppConfig,
) -> (
    UnboundedReceiver<ServerMessage>,
    TwitchIRCClient<twitch_irc::SecureTCPTransport, RefreshingLoginCredentials<InMemoryTokenStorage>>,
) {
    let auth_callback_url = {
        let mut x = app_config.server_url.clone();
        x.set_path("auth");
        x
    };

    let mut user_token_builder = UserTokenBuilder::new(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        auth_callback_url,
    )
    .set_scopes(vec![twitch_oauth2::Scope::ChatRead, twitch_oauth2::Scope::ChatEdit]);

    let (auth_url, _) = user_token_builder.generate_url();

    webbrowser::open(auth_url.as_str()).unwrap();

    let app_state = Arc::new(AppState {
        auth_response_step_1: Mutex::new(None),
    });

    let app = Router::new()
        .route("/auth", get(auth_callback))
        .with_state(app_state.clone());

    tokio::spawn(async move {
        axum::Server::bind(&app_config.socket_addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    });

    loop {
        if app_state.auth_response_step_1.lock().await.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    let AuthResponseStep1 { code, state } = app_state.auth_response_step_1.lock().await.clone().unwrap();
    let client = reqwest::Client::new();

    let user_token = user_token_builder
        .get_user_token(&client, state.as_str(), code.as_str())
        .await
        .unwrap();

    let get_access_token_response = GetAccessTokenResponse {
        access_token: user_token.access_token.secret().into(),
        refresh_token: user_token.refresh_token.clone().unwrap().secret().into(),
        expires_in: Some(user_token.expires_in().as_secs()),
    };

    let custom_token_storage = InMemoryTokenStorage(get_access_token_response.into());

    let credentials = RefreshingLoginCredentials::init(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        custom_token_storage,
    );

    let client_config = twitch_irc::ClientConfig::new_simple(credentials);
    TwitchIRCClient::<twitch_irc::SecureTCPTransport, RefreshingLoginCredentials<InMemoryTokenStorage>>::new(
        client_config,
    )
}

async fn auth_callback(State(app_state): State<Arc<AppState>>, Query(auth_response_step_1): Query<AuthResponseStep1>) {
    let mut guard = app_state.auth_response_step_1.lock().await;
    *guard = Some(auth_response_step_1.clone());
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub socket_addr: SocketAddr,
    pub server_url: Url,
    pub database_url: Url,
    pub client_id: String,
    pub client_secret: String,
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

struct AppState {
    auth_response_step_1: Mutex<Option<AuthResponseStep1>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthResponseStep1 {
    pub code: String,
    pub state: String,
}

#[derive(Debug)]
pub struct InMemoryTokenStorage(UserAccessToken);

#[async_trait]
impl TokenStorage for InMemoryTokenStorage {
    type LoadError = std::io::Error;
    type UpdateError = std::io::Error;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        Ok(self.0.clone())
    }

    async fn update_token(&mut self, token: &UserAccessToken) -> Result<(), Self::UpdateError> {
        self.0 = token.clone();
        Ok(())
    }
}
