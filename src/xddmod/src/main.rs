use sqlx::SqlitePool;
use twitch_api2::HelixClient;
use xddmod::auth;
use xddmod::auth::AppConfig;
use xddmod::handlers::gambage::core::Gambage;
use xddmod::handlers::npc::core::Npc;

#[tokio::main]
async fn main() {
    let app_config = AppConfig::init();
    let channel = std::env::args().nth(1).unwrap();
    let you = std::env::args().nth(2).unwrap().to_lowercase();

    let db_pool = SqlitePool::connect(app_config.database_url.as_ref()).await.unwrap();

    let (mut incoming_messages, irc_client, user_token) = auth::authenticate(app_config.clone()).await;
    let helix_client: HelixClient<'static, reqwest::Client> = HelixClient::default();

    let broadcaster = helix_client
        .get_user_from_login(channel.to_string(), &user_token)
        .await
        .unwrap()
        .unwrap();

    irc_client.join(channel).unwrap();

    let templates_env = xddmod::templates_env::build_global_templates_env();

    let npc = Npc {
        you,
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    };
    let gambage = Gambage {
        token: user_token,
        broadcaster_id: broadcaster.id,
        helix_client,
        irc_client,
        db_pool,
        templates_env,
    };

    #[allow(clippy::single_match)]
    tokio::spawn(async move {
        while let Some(server_message) = incoming_messages.recv().await {
            npc.handle(&server_message).await;
            gambage.handle(&server_message).await;
        }
    })
    .await
    .unwrap();
}
