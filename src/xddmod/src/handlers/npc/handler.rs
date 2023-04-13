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
    pub async fn handle(&self, server_message: &ServerMessage) {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            match Reply::matching(
                Handler::Npc,
                &self.you,
                &message.channel_login,
                &message.message_text,
                &self.db_pool,
            )
            .await
            .as_slice()
            {
                [reply] => match reply.render_template(&self.templates_env) {
                    Ok(expaned_reply) if expaned_reply.is_empty() => {
                        println!("Expanded reply template empty: {:?}", reply)
                    }
                    Ok(expaned_reply) => self.irc_client.say_in_reply_to(message, expaned_reply).await.unwrap(),
                    Err(e) => println!("Error expanding reply template, error: {:?}, {:?}.", reply, e),
                },
                [] => {}
                multiple_matchin_replies => println!(
                    "Multiple matching replies for message: {:?}, {:?}.",
                    multiple_matchin_replies, server_message
                ),
            }
        }
    }
}
