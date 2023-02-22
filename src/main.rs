use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use axum::async_trait;
use axum::extract::Query;
use axum::extract::State;
use axum::response::Redirect;
use axum::routing::get;
use axum::routing::Router;
use config::Config;
use config::Environment;
use hyper::header::CONTENT_TYPE;
use hyper::Body;
use hyper::Client;
use hyper::Request;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use twitch_irc::login::GetAccessTokenResponse;
use twitch_irc::login::RefreshingLoginCredentials;
use twitch_irc::login::TokenStorage;
use twitch_irc::login::UserAccessToken;
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
        db_pool: db_pool.clone(),
        config: app_config.clone(),
        credentials,
    });

    let host = app_state.config.host.clone();

    let app = Router::new()
        .route("/", get(auth_callback))
        .with_state(app_state);

    axum::Server::bind(&Socket::try_from(&host).unwrap().into())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

struct Socket(SocketAddr);

impl TryFrom<&Url> for Socket {
    type Error = anyhow::Error;

    fn try_from(x: &Url) -> Result<Self, Self::Error> {
        Ok(Self(
            x.host_str()
                .and_then(|h| x.port().map(|p| format!("{}:{}", h, p)))
                .ok_or_else(|| anyhow::anyhow!("Error creating SocketAddr from Url {:?}", x))?
                .parse::<SocketAddr>()?,
        ))
    }
}

impl From<Socket> for SocketAddr {
    fn from(x: Socket) -> Self {
        x.0
    }
}

async fn auth_callback<T: TokenStorage>(
    State(app_state): State<Arc<AppState<T>>>,
    Query(auth_result): Query<AuthResult>,
) -> Redirect {
    let SuccessfulAuth { code, .. } =
        Result::from(auth_result).unwrap_or_else(|failed_auth| panic!("{:?}", failed_auth));

    let request = AuthRequest {
        client_id: app_state.config.client_id.clone(),
        client_secret: app_state.config.client_secret.clone(),
        code,
        grant_type: "authorization_code".into(),
        redirect_uri: app_state.config.host.clone(),
    };

    let request = Request::post::<&str>(app_state.config.twitch_oauth2_endpoint.as_ref())
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(serde_json::to_string(&request).unwrap()))
        .unwrap();

    let response = Client::new().request(request).await.unwrap();
    let body = serde_json::from_slice::<GetAccessTokenResponse>(
        &hyper::body::to_bytes(response.into_body()).await.unwrap(),
    )
    .unwrap();

    Redirect::to("/start")
}

#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    host: Url,
    database_url: Url,
    token_file_path: PathBuf,
    client_id: String,
    client_secret: String,
    twitch_oauth2_endpoint: Url,
}

#[derive(Debug)]
struct AppState<T: TokenStorage> {
    token_storage: TokenFileStorage,
    db_pool: SqlitePool,
    config: AppConfig,
    credentials: RefreshingLoginCredentials<T>,
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

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AuthResult {
    Success(SuccessfulAuth),
    Fail(FailedAuth),
}

#[derive(Debug, Deserialize)]
struct SuccessfulAuth {
    code: String,
    _scope: String,
    _state: String,
}

#[derive(Debug, Deserialize)]
struct FailedAuth {
    _error: String,
    _error_description: String,
    _state: String,
}

impl From<AuthResult> for Result<SuccessfulAuth, FailedAuth> {
    fn from(x: AuthResult) -> Self {
        match x {
            AuthResult::Fail(failed_auth) => Err(failed_auth),
            AuthResult::Success(successful_auth) => Ok(successful_auth),
        }
    }
}

#[derive(Debug, Serialize)]
struct AuthRequest {
    client_id: String,
    client_secret: String,
    code: String,
    grant_type: String,
    redirect_uri: Url,
}
