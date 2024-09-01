use std::sync::mpsc::Sender;
use std::sync::Arc;

use axum::async_trait;
use axum::extract::Query;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::response::Redirect;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Mutex;
use twitch_api::twitch_oauth2::tokens::UserTokenBuilder;
use twitch_api::twitch_oauth2::Scope;
use twitch_api::twitch_oauth2::TwitchToken;
use twitch_api::twitch_oauth2::UserToken;
use twitch_irc::login::GetAccessTokenResponse;
use twitch_irc::login::RefreshingLoginCredentials;
use twitch_irc::login::TokenStorage;
use twitch_irc::login::UserAccessToken;
use twitch_irc::message::ServerMessage;
use twitch_irc::SecureTCPTransport;
use twitch_irc::TwitchIRCClient;

use crate::app_config::AppConfig;

pub type MessageReceiver = UnboundedReceiver<ServerMessage>;
pub type IRCClient = TwitchIRCClient<SecureTCPTransport, RefreshingLoginCredentials<InMemoryTokenStorage>>;

pub async fn authenticate<'a>(app_config: AppConfig) -> (MessageReceiver, IRCClient, UserToken) {
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
    .set_scopes(vec![
        Scope::ChatRead,
        Scope::ChatEdit,
        Scope::ChannelReadPredictions,
        Scope::ModeratorManageBannedUsers,
        Scope::parse("moderator:manage:chat_messages"),
    ]);

    let (auth_url, _) = user_token_builder.generate_url();

    webbrowser::open(auth_url.as_str()).unwrap();

    let (sender, receiver) = std::sync::mpsc::channel::<AuthResponseStep1>();
    let app_state = Arc::new(Mutex::new(sender));

    let app = Router::new()
        .route("/auth", get(auth_callback))
        .with_state(app_state.clone());

    let listener = TcpListener::bind(&app_config.socket_addr).await.unwrap();
    let auth_server = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });

    let AuthResponseStep1 { code, state } = receiver.recv().unwrap();

    auth_server.abort();
    auth_server.await.unwrap_err().is_cancelled();

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
    State(sender): State<Arc<Mutex<Sender<AuthResponseStep1>>>>,
    Query(auth_response_step_1): Query<AuthResponseStep1>,
) -> Response {
    let sender = sender.lock().await;
    sender.send(auth_response_step_1).unwrap();
    Redirect::to("https://cdn.7tv.app/emote/63bb3450799f5d0ce4b80686/4x.webp").into_response()
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
