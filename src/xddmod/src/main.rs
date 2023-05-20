use sqlx::SqlitePool;
use twitch_api::HelixClient;
use xddmod::app_config::AppConfig;
use xddmod::auth;
use xddmod::handlers::gg::core::Gg;
use xddmod::handlers::npc::core::Npc;
use xddmod::handlers::rip_bozo::core::RipBozo;
use xddmod::handlers::sniffa::core::Sniffa;

#[tokio::main]
async fn main() {
    let app_config = AppConfig::init();
    let channel = std::env::args().nth(1).unwrap();

    let db_pool = SqlitePool::connect(app_config.database_url.as_ref()).await.unwrap();

    let (mut incoming_messages, irc_client, user_token) = auth::authenticate(app_config.clone()).await;

    let helix_client: HelixClient<'static, reqwest::Client> = HelixClient::default();
    let broadcaster = helix_client
        .get_user_from_login(&channel, &user_token)
        .await
        .unwrap()
        .unwrap();

    irc_client.join(channel).unwrap();

    let templates_env = xddmod::templates_env::build_global_templates_env();

    let npc = Npc {
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    };
    let gg = Gg {
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    };
    let sniffa = Sniffa {
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    };
    let mut rip_bozo = RipBozo {
        broadcaster_id: broadcaster.id,
        token: user_token,
        helix_client,
        db_pool,
    };

    #[allow(clippy::single_match)]
    tokio::spawn(async move {
        while let Some(server_message) = incoming_messages.recv().await {
            rip_bozo.handle(&server_message).await;
            npc.handle(&server_message).await;
            gg.handle(&server_message).await;
            sniffa.handle(&server_message).await;
        }
    })
    .await
    .unwrap();
}
