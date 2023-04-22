use minijinja::value::Value;
use minijinja::Environment;
use sqlx::SqlitePool;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;

pub struct Npc<'a> {
    pub you: String,
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl<'a> Npc<'a> {
    pub fn handler(&self) -> Handler {
        Handler::Npc
    }
}

impl<'a> Npc<'a> {
    pub async fn handle(&self, server_message: &ServerMessage) {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            match Reply::matching(
                self.handler(),
                Some(&self.you),
                &message.channel_login,
                &message.message_text,
                &self.db_pool,
            )
            .await
            .as_slice()
            {
                [reply] => {
                    // FIXME: poor man throttling
                    match should_throttle(message, reply) {
                        Ok(false) => (),
                        Ok(true) => {
                            eprintln!(
                                "Skipping reply: message {:?}, sender {:?}, reply {:?}",
                                message.message_text, message.sender, reply.template
                            );
                            return;
                        }
                        Err(error) => {
                            eprintln!("Error throttling, error: {:?}", error);
                        }
                    }

                    match reply.render_template::<Value>(&self.templates_env, None) {
                        Ok(expaned_reply) if expaned_reply.is_empty() => {
                            eprintln!("Expanded reply template empty: {:?}", reply)
                        }
                        Ok(expaned_reply) => self.irc_client.say_in_reply_to(message, expaned_reply).await.unwrap(),
                        Err(e) => eprintln!("Error expanding reply template, error: {:?}, {:?}.", reply, e),
                    }
                }
                [] => {}
                multiple_matchin_replies => eprintln!(
                    "Multiple matching replies for message: {:?}, {:?}.",
                    multiple_matchin_replies, server_message
                ),
            }
        }
    }
}

// FIXME: poor man throttling
lazy_static::lazy_static! {
    static ref THROTTLE: std::sync::Mutex<
        std::collections::BTreeMap<i64, sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>
    >  = std::sync::Mutex::new(std::collections::BTreeMap::new());
}

fn should_throttle(message: &PrivmsgMessage, reply: &Reply) -> anyhow::Result<bool> {
    if message
        .badges
        .iter()
        .any(|b| b.name == "moderator" || b.name == "broadcaster")
    {
        return Ok(false);
    }

    let mut throttle = THROTTLE
        .lock()
        .map_err(|error| anyhow::anyhow!("Cannot get THROTTLE Lock, error: {:?}", error))?;

    let throttling = throttle
        .get(&reply.id)
        .map(|last_reply_date_time| {
            let time_passed = sqlx::types::chrono::Utc::now() - *last_reply_date_time;
            time_passed < chrono::Duration::seconds(10)
        })
        .unwrap_or_default();

    if !throttling {
        throttle.insert(reply.id, sqlx::types::chrono::Utc::now());
    }

    Ok(throttling)
}
