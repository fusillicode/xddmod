use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::async_trait;
use axum::extract::Query;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::response::Redirect;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use config::Config;
use config::Environment;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Mutex;
use twitch_api2::twitch_oauth2::tokens::UserTokenBuilder;
use twitch_api2::twitch_oauth2::ClientId;
use twitch_api2::twitch_oauth2::ClientSecret;
use twitch_api2::twitch_oauth2::Scope;
use twitch_api2::twitch_oauth2::TwitchToken;
use twitch_irc::login::GetAccessTokenResponse;
use twitch_irc::login::RefreshingLoginCredentials;
use twitch_irc::login::TokenStorage;
use twitch_irc::login::UserAccessToken;
use twitch_irc::message::ServerMessage;
use twitch_irc::SecureTCPTransport;
use twitch_irc::TwitchIRCClient;
use url::Url;

pub type MessageReceiver = UnboundedReceiver<ServerMessage>;
pub type IRCClient = TwitchIRCClient<SecureTCPTransport, RefreshingLoginCredentials<InMemoryTokenStorage>>;

pub async fn authenticate<'a>(app_config: AppConfig) -> (MessageReceiver, IRCClient, impl TwitchToken) {
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
    .set_scopes(vec![Scope::ChatRead, Scope::ChatEdit]);

    let (auth_url, _) = user_token_builder.generate_url();

    webbrowser::open(auth_url.as_str()).unwrap();

    let app_state = Arc::new(AppState {
        auth_response_step_1: Mutex::new(None),
    });

    let app = Router::new()
        .route("/auth", get(auth_callback))
        .with_state(app_state.clone());

    let auth_server = tokio::spawn(async move {
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

    auth_server.abort();
    auth_server.await.unwrap_err().is_cancelled();

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
        app_config.client_id.to_string(),
        app_config.client_secret.secret().to_string(),
        custom_token_storage,
    );

    let client_config = twitch_irc::ClientConfig::new_simple(credentials);
    let (messages_receiver, irc_client) =
        TwitchIRCClient::<SecureTCPTransport, RefreshingLoginCredentials<InMemoryTokenStorage>>::new(client_config);

    (messages_receiver, irc_client, user_token)
}

async fn auth_callback(
    State(app_state): State<Arc<AppState>>,
    Query(auth_response_step_1): Query<AuthResponseStep1>,
) -> Response {
    let mut guard = app_state.auth_response_step_1.lock().await;
    *guard = Some(auth_response_step_1.clone());
    Redirect::to("https://cdn.7tv.app/emote/63bb3450799f5d0ce4b80686/4x.webp").into_response()
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub socket_addr: SocketAddr,
    pub server_url: Url,
    pub database_url: Url,
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
