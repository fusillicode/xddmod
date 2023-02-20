use axum::extract::Query;
use axum::routing::get;
use axum::routing::Router;
use serde::Deserialize;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(auth));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
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
    scope: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct FailedAuth {
    error: String,
    error_description: String,
    state: String,
}

async fn auth(Query(auth_result): Query<AuthResult>) -> String {
    dbg!(auth_result);
    "foo".into()
}
