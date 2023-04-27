use minijinja::context;
use minijinja::Environment;
use sqlx::SqlitePool;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::handlers::gg::op_gg_client;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;

pub struct Gg<'a> {
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl<'a> Gg<'a> {
    pub fn handler(&self) -> Handler {
        Handler::Gamba
    }
}

impl<'a> Gg<'a> {
    pub async fn handle(&self, server_message: &ServerMessage) {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            match Reply::matching(
                self.handler(),
                None,
                &message.channel_login,
                &message.message_text,
                &self.db_pool,
            )
            .await
            .as_slice()
            {
                [reply] => (),
                [] => {}
                multiple_matchin_replies => eprintln!(
                    "Multiple matching replies for message: {:?}, {:?}.",
                    multiple_matchin_replies, server_message
                ),
            }
        }
    }
}
