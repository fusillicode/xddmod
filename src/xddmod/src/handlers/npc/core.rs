use minijinja::value::Value;
use minijinja::Environment;
use sqlx::SqlitePool;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;
use crate::poor_man_throttling;

pub struct Npc<'a> {
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
                &message.channel_login,
                &message.message_text,
                &self.db_pool,
            )
            .await
            .as_slice()
            {
                [reply] => {
                    // FIXME: poor man throttling
                    match poor_man_throttling::should_throttle(message, reply) {
                        Ok(false) => (),
                        Ok(true) => {
                            eprintln!(
                                "Skip reply: message {:?}, sender {:?}, reply {:?}",
                                message.message_text, message.sender, reply.template
                            );
                            return;
                        }
                        Err(error) => {
                            eprintln!("Error throttling, error: {:?}", error);
                        }
                    }

                    match reply.render_template::<Value>(&self.templates_env, None) {
                        Ok(rendered_reply) if rendered_reply.is_empty() => {
                            eprintln!("Rendered reply template empty: {:?}", reply)
                        }
                        Ok(rendered_reply) => self.irc_client.say_in_reply_to(message, rendered_reply).await.unwrap(),
                        Err(e) => eprintln!("Error rendering reply template, error: {:?}, {:?}.", reply, e),
                    }
                }
                [] => {}
                multiple_matching_replies => eprintln!(
                    "Multiple matching replies for message: {:?}, {:?}.",
                    multiple_matching_replies, server_message
                ),
            }
        }
    }
}
