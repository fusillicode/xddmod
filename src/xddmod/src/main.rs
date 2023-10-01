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
async fn main() -> anyhow::Result<()> {
    let app_config = AppConfig::init();
    let channel = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("missing 1st CLI arg, `channel`"))?;

    let db_pool = SqlitePool::connect(app_config.db_url.as_ref()).await?;

    let (mut incoming_messages, irc_client, user_token) = auth::authenticate(app_config.clone()).await;

    let helix_client: HelixClient<'static, reqwest::Client> = HelixClient::default();

    let broadcaster = helix_client
        .get_user_from_login(&channel, &user_token)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no broacaster found for `channel` {} with `user_token` {:?}",
                channel,
                user_token
            )
        })?;

    irc_client.join(channel)?;

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
                let mut rip_bozo_guard = rip_bozo.lock().await;
                if let Ok(true) = rip_bozo_guard.handle(&server_message).await {
                    return;
                }
                if let Err(e) = npc.handle(&server_message).await {
                    eprintln!("{} error {:?}", npc.handler(), e);
                };
                if let Err(e) = gg.handle(&server_message).await {
                    eprintln!("{} error {:?}", gg.handler(), e);
                };
                if let Err(e) = sniffa.handle(&server_message).await {
                    eprintln!("{} error {:?}", sniffa.handler(), e);
                };
                if let Err(e) = the_grind.handle(&server_message).await {
                    eprintln!("{} error {:?}", the_grind.handler(), e);
                };
            });
        }
    })
    .await?;

    Ok(())
}
