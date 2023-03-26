use sqlx::SqlitePool;

mod auth;
mod persistence;

use crate::auth::AppConfig;
use crate::persistence::Cmd;
use crate::persistence::NewCmd;

#[tokio::main]
async fn main() {
    let app_config = AppConfig::init();
    let channels = std::env::args()
        .nth(1)
        .unwrap()
        .split(',')
        .map(String::from)
        .collect::<Vec<String>>();
    let you = std::env::args().nth(2).unwrap();

    let db_pool = SqlitePool::connect(app_config.database_url.as_ref()).await.unwrap();
    sqlx::migrate!().run(&db_pool).await.unwrap();

    let (mut incoming_messages, client) = auth::authenticate(app_config.clone()).await;

    for channel in channels {
        client.join(channel).unwrap();
    }

    #[allow(clippy::single_match)]
    tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                twitch_irc::message::ServerMessage::Privmsg(message) => {
                    if message.message_text.to_lowercase().contains(&you) {
                        if message.message_text.to_lowercase().contains("gigachad")
                            || message.message_text.to_lowercase().contains("best mod")
                            || message.message_text.to_lowercase().contains("<3")
                        {
                            if let Some(cmd) = Cmd::last_by_shortcut("!nah", &db_pool).await.unwrap() {
                                client.say_in_reply_to(&message, cmd.expansion).await.unwrap();
                            }
                            continue;
                        }

                        if message.message_text.to_lowercase().contains("thank you")
                            || message.message_text.to_lowercase().contains("thnx")
                        {
                            if let Some(cmd) = Cmd::last_by_shortcut("!np", &db_pool).await.unwrap() {
                                client.say_in_reply_to(&message, cmd.expansion).await.unwrap();
                            }
                            continue;
                        }
                    }

                    if !message.message_text.starts_with('!') {
                        continue;
                    }

                    match message.message_text.split_once(' ') {
                        Some((shortcut, expansion)) => {
                            dbg!(&message);
                            if !message
                                .badges
                                .iter()
                                .any(|x| x.name == "moderator" || x.name == "admin" || x.name == "broadcaster")
                            {
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
                            if let Some(cmd) = Cmd::last_by_shortcut(&message.message_text, &db_pool).await.unwrap() {
                                client.say_in_reply_to(&message, cmd.expansion).await.unwrap();
                            }
                        }
                    }
                }

                _ => {}
            }
        }
    })
    .await
    .unwrap();
}
