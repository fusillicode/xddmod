use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;
use twitch_api::HelixClient;
use xddmod::app_config::AppConfig;
use xddmod::auth;
use xddmod::handlers::gg::core::Gg;
use xddmod::handlers::npc::core::Npc;
use xddmod::handlers::rip_bozo::core::RipBozo;
use xddmod::handlers::sniffa::core::Sniffa;
use xddmod::handlers::the_grind::core::TheGrind;

#[tokio::main]
async fn main() {
    let app_config = AppConfig::init();
    let channel = std::env::args().nth(1).unwrap();

    let db_pool = SqlitePool::connect(app_config.db_url.as_ref()).await.unwrap();

    let (mut incoming_messages, irc_client, user_token) = auth::authenticate(app_config.clone()).await;

    let helix_client: HelixClient<'static, reqwest::Client> = HelixClient::default();

    let broadcaster = helix_client
        .get_user_from_login(&channel, &user_token)
        .await
        .unwrap()
        .unwrap();

    irc_client.join(channel).unwrap();

    let templates_env = xddmod::templates_env::build_global_templates_env();

    let rip_bozo = Arc::new(Mutex::new(RipBozo {
        broadcaster_id: broadcaster.id,
        token: user_token,
        helix_client,
        db_pool: db_pool.clone(),
    }));
    let npc = Arc::new(Npc {
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    });
    let gg = Arc::new(Gg {
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    });
    let sniffa = Arc::new(Sniffa {
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    });
    let the_grind = Arc::new(TheGrind {
        irc_client: irc_client.clone(),
        db_pool: db_pool.clone(),
        templates_env: templates_env.clone(),
    });

    #[allow(clippy::single_match)]
    tokio::spawn(async move {
        while let Some(server_message) = incoming_messages.recv().await {
            let rip_bozo = rip_bozo.clone();
            let npc = npc.clone();
            let gg = gg.clone();
            let sniffa = sniffa.clone();
            let the_grind = the_grind.clone();

            tokio::spawn(async move {
                let mut rip_bozo_g = rip_bozo.lock().await;
                if let Ok(true) = rip_bozo_g.handle(&server_message).await {
                    return;
                }
                if let Err(e) = npc.handle::<reqwest::Error>(&server_message).await {
                    eprintln!("Npc error {:?}", e);
                };
                gg.handle(&server_message).await;
                sniffa.handle(&server_message).await;
                the_grind.handle(&server_message).await;
            });
        }
    })
    .await
    .unwrap();
}
