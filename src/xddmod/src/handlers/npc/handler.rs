use sqlx::SqlitePool;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::handlers::npc::persistence::NpcReply;
use crate::persistence::Channel;

pub struct Npc {
    pub you: String,
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
}

impl Npc {
    pub async fn handle(&self, server_message: &ServerMessage) {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            let channel = Channel::get(&message.channel_login, &self.db_pool).await.unwrap();
            match NpcReply::matching(&self.you, &message.channel_login, &message.message_text, &self.db_pool)
                .await
                .as_slice()
            {
                [reply] => self
                    .irc_client
                    .say_in_reply_to(message, reply.expand_with(&channel))
                    .await
                    .unwrap(),
                [] => println!("No matching replies for message: {:?}.", server_message),
                multiple_matchin_replies => println!(
                    "Multiple matching replies for message: {:?}, {:?}.",
                    multiple_matchin_replies, server_message
                ),
            }
        }
    }
}
