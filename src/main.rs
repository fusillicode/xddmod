use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::async_trait;
use axum::extract::Query;
use axum::extract::State;
use axum::routing::get;
use axum::routing::Router;
use config::Config;
use config::Environment;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use twitch_irc::login::GetAccessTokenResponse;
use twitch_irc::login::RefreshingLoginCredentials;
use twitch_irc::login::TokenStorage;
use twitch_irc::login::UserAccessToken;
use twitch_irc::TwitchIRCClient;
use twitch_oauth2::tokens::UserTokenBuilder;
use twitch_oauth2::TwitchToken;
use url::Url;

#[tokio::main]
async fn main() {
    let app_config: AppConfig = Config::builder()
        .add_source(Environment::default())
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap();

    let db_pool = SqlitePool::connect(app_config.database_url.as_ref())
        .await
        .unwrap();
    sqlx::migrate!().run(&db_pool).await.unwrap();

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
        twitch_oauth2::Scope::ChatRead,
        twitch_oauth2::Scope::ChatEdit,
    ]);

    let (auth_url, _) = user_token_builder.generate_url();

    println!("GET THE FUCK HERE {}", auth_url);

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
        println!("WAITING...");
    }

    let AuthResponseStep1 { code, state } =
        app_state.auth_response_step_1.lock().await.clone().unwrap();
    let client = reqwest::Client::new();

    let user_token = user_token_builder
        .get_user_token(&client, state.as_str(), code.as_str())
        .await
        .unwrap();

    // let http_client = reqwest::Client::new();
    // let response = http_client
    //     .execute(ciccia.try_into().unwrap())
    //     .await
    //     .unwrap();
    // let merda = axum::http::Response::builder()
    //     .status(200)
    //     .body(response.bytes().await.unwrap())
    //     .unwrap();
    // let twitch_response = TwitchTokenResponse::from_response(&merda).unwrap();
    // dbg!(&twitch_response);

    let get_access_token_response = GetAccessTokenResponse {
        access_token: user_token.access_token.to_string(),
        refresh_token: user_token.refresh_token.clone().unwrap().to_string(),
        expires_in: Some(user_token.expires_in().as_secs()),
    };

    let custom_token_storage = CustomTokenStorage(get_access_token_response.into());

    let credentials = RefreshingLoginCredentials::init(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        custom_token_storage,
    );

    let client_config = twitch_irc::ClientConfig::new_simple(credentials);
    let (mut incoming_messages, client) = TwitchIRCClient::<
        twitch_irc::SecureTCPTransport,
        RefreshingLoginCredentials<CustomTokenStorage>,
    >::new(client_config);

    dbg!("MERDA");

    // client.join("fusillicode".to_owned()).unwrap();
    dbg!(client.get_channel_status("fusillicode".to_owned()).await);
    client
        .say("fusillicode".into(), "xdd".into())
        .await
        .unwrap();

    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            println!("Received message: {:?}", message);
        }
    });

    join_handle.await.unwrap();
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthResponseStep1 {
    pub code: String,
    pub state: String,
}

async fn auth_callback(
    State(app_state): State<Arc<AppState>>,
    Query(auth_response_step_1): Query<AuthResponseStep1>,
) {
    dbg!(&auth_response_step_1);
    let mut guard = app_state.auth_response_step_1.lock().await;
    *guard = Some(auth_response_step_1.clone());
}

#[derive(Debug)]
struct CustomTokenStorage(UserAccessToken);

#[async_trait]
impl TokenStorage for CustomTokenStorage {
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

#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    socket_addr: SocketAddr,
    server_url: Url,
    database_url: Url,
    client_id: String,
    client_secret: String,
}

struct AppState {
    auth_response_step_1: Mutex<Option<AuthResponseStep1>>,
}
