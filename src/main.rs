use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use axum::async_trait;
use axum::extract::State;
use axum::response::Redirect;
use axum::routing::get;
use axum::routing::Router;
use config::Config;
use config::Environment;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use twitch_irc::login::GetAccessTokenResponse;
use twitch_irc::login::RefreshingLoginCredentials;
use twitch_irc::login::TokenStorage;
use twitch_irc::login::UserAccessToken;
use twitch_oauth2::tokens::UserTokenBuilder;
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

    let token_storage = TokenFileStorage::init(app_config.token_file_path.as_path())
        .await
        .unwrap();

    let credentials = RefreshingLoginCredentials::init(
        app_config.client_id.clone(),
        app_config.client_secret.clone(),
        token_storage.try_clone().await.unwrap(),
    );

    let app_state = Arc::new(AppState {
        token_storage,
        credentials,
        db_pool: db_pool.clone(),
        config: app_config.clone(),
    });

    let app = Router::new()
        .route("/start", get(start))
        .route("/auth", get(auth_callback))
        .with_state(app_state);

    axum::Server::bind(&app_config.socket_addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn start<T: TokenStorage>(State(app_state): State<Arc<AppState<T>>>) -> Redirect {
    let auth_callback_url = {
        let mut x = app_state.config.server_url.clone();
        x.set_path("auth");
        x
    };

    let mut user_token_builder = UserTokenBuilder::new(
        app_state.config.client_id.clone(),
        app_state.config.client_secret.clone(),
        auth_callback_url,
    );

    let (auth_url, _) = user_token_builder.generate_url();

    Redirect::to(auth_url.as_str())
}

async fn auth_callback<T: TokenStorage>(
    State(app_state): State<Arc<AppState<T>>>,
    // Query(auth_result): Query<AuthResult>,
) -> Redirect {
    Redirect::to("/start")
}

#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    socket_addr: SocketAddr,
    server_url: Url,
    database_url: Url,
    token_file_path: PathBuf,
    client_id: String,
    client_secret: String,
}

struct AppState<T: TokenStorage> {
    token_storage: TokenFileStorage,
    credentials: RefreshingLoginCredentials<T>,
    db_pool: SqlitePool,
    config: AppConfig,
}

#[derive(Debug)]
struct TokenFileStorage(File);

impl TokenFileStorage {
    pub async fn init(file_path: &Path) -> std::io::Result<Self> {
        file_path.try_exists()?;
        Ok(Self(File::open(file_path).await?))
    }

    pub async fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self(self.0.try_clone().await?))
    }
}

#[async_trait]
impl TokenStorage for TokenFileStorage {
    type LoadError = std::io::Error;
    type UpdateError = std::io::Error;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        let mut file_content = String::new();
        self.0.read_to_string(&mut file_content).await?;
        Ok(serde_json::from_str::<UserAccessToken>(&file_content).unwrap())
    }

    async fn update_token(&mut self, token: &UserAccessToken) -> Result<(), Self::UpdateError> {
        self.0
            .write_all(&serde_json::to_vec(&token).unwrap())
            .await?;
        Ok(())
    }
}
