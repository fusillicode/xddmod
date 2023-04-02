use sqlx::SqlitePool;

mod auth;
mod cooks;
mod persistence;

use crate::auth::AppConfig;
use crate::cooks::npc::Npc;

#[tokio::main]
async fn main() {
    let app_config = AppConfig::init();
    let channels = std::env::args()
        .nth(1)
        .unwrap()
        .split(',')
        .map(String::from)
        .collect::<Vec<String>>();
    let you = std::env::args().nth(2).unwrap().to_lowercase();

    let db_pool = SqlitePool::connect(app_config.database_url.as_ref()).await.unwrap();

    let (mut incoming_messages, irc_client, _token) = auth::authenticate(app_config.clone()).await;

    for channel in channels {
        irc_client.join(channel).unwrap();
    }

    let npc = Npc {
        you,
        irc_client,
        db_pool,
    };

    #[allow(clippy::single_match)]
    tokio::spawn(async move {
        while let Some(server_message) = incoming_messages.recv().await {
            npc.let_me_cook(&server_message).await
        }
    })
    .await
    .unwrap();
}
