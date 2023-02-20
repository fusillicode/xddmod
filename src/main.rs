use std::sync::Arc;

use axum::extract::Query;
use axum::extract::State;
use axum::response::Redirect;
use axum::routing::get;
use axum::routing::Router;
use config::Config;
use config::Environment;
use hyper::header::CONTENT_TYPE;
use hyper::Request;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use url::Url;

#[tokio::main]
async fn main() {
    let app_config: AppConfig = Config::builder()
        .add_source(Environment::default())
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap();

    dbg!(&app_config);

    let db_pool = SqlitePool::connect(app_config.database_url.as_ref())
        .await
        .unwrap();
    sqlx::migrate!().run(&db_pool).await.unwrap();

    let app_state = Arc::new(AppState {
        db_pool,
        config: app_config,
    });

    let app = Router::new().route("/", get(auth)).with_state(app_state);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn auth(
    State(app_state): State<Arc<AppState>>,
    Query(auth_result): Query<AuthResult>,
) -> Redirect {
    let SuccessfulAuth { code, .. } =
        Result::from(auth_result).unwrap_or_else(|failed_auth| panic!("{:?}", failed_auth));

    Request::post::<&str>(app_state.config.twitch_oauth2_endpoint.as_ref())
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(AuthRequest {
            client_id: app_state.config.client_id.clone(),
            client_secret: app_state.config.client_secret.clone(),
            code,
            grant_type: "authorization_code".into(),
            redirect_uri: app_state.config.host.clone(),
        })
        .unwrap();

    Redirect::to("/start")
}

#[derive(Debug, Deserialize)]
struct AppConfig {
    host: Url,
    database_url: Url,
    client_id: String,
    client_secret: String,
    twitch_oauth2_endpoint: Url,
}

#[derive(Debug)]
struct AppState {
    db_pool: SqlitePool,
    config: AppConfig,
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
