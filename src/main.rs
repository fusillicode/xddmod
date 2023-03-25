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
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;
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
    let channel_name = std::env::args().nth(1).unwrap();

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

    println!("\n{}", auth_url);

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

    println!("\n 🚀");

    let AuthResponseStep1 { code, state } =
        app_state.auth_response_step_1.lock().await.clone().unwrap();
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

    client.join(channel_name).unwrap();

    #[allow(clippy::single_match)]
    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                twitch_irc::message::ServerMessage::Privmsg(message) => {
                    if !message.message_text.starts_with('!') {
                        continue;
                    }

                    match message.message_text.split_once(' ') {
                        Some((shortcut, expansion)) => {
                            dbg!(&message);
                            if !message.badges.iter().any(|x| {
                                x.name == "moderator"
                                    || x.name == "admin"
                                    || x.name == "broadcaster"
                            }) {
                                continue;
                            }
                            NewCmd {
                                shortcut: shortcut.into(),
                                expansion: expansion.into(),
                                created_by: message.sender.login,
                            }
                            .insert(&db_pool)
                            .await
                            .unwrap();
                        }
                        None => {
                            if let Some(cmd) =
                                Cmd::last_by_shortcut(&message.message_text, &db_pool)
                                    .await
                                    .unwrap()
                            {
                                client
                                    .say_in_reply_to(&message, cmd.expansion)
                                    .await
                                    .unwrap();
                            }
                        }
                    }
                }

                _ => {}
            }
        }
    });

    join_handle.await.unwrap();
}

pub struct NewCmd {
    pub shortcut: String,
    pub expansion: String,
    pub created_by: String,
}

impl NewCmd {
    pub async fn insert<'a>(
        &self,
        executor: impl sqlx::sqlite::SqliteExecutor<'a>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "insert into cmds (shortcut, expansion, created_by) values ($1, $2, $3)",
            self.shortcut,
            self.expansion,
            self.created_by,
        )
        .execute(executor)
        .await
        .map(|_| ())?;

        Ok(())
    }
}

pub struct Cmd {
    pub id: i64,
    pub shortcut: String,
    pub expansion: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

impl Cmd {
    pub async fn last_by_shortcut<'a>(
        shortcut: &str,
        executor: impl sqlx::sqlite::SqliteExecutor<'a>,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
                select id, shortcut, expansion, created_by, created_at as "created_at!: DateTime<Utc>"
                from cmds
                where shortcut = $1
                order by id desc
            "#,
            shortcut,
        )
        .fetch_optional(executor)
        .await
    }
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
