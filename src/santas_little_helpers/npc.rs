use regex::RegexBuilder;
use sqlx::types::chrono::Utc;
use sqlx::SqlitePool;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::persistence::{Channel, Reply};

pub struct Npc {
    pub you: String,
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
}

impl Npc {
    pub async fn let_me_cook(&self, server_message: &ServerMessage) {
        if let ServerMessage::Privmsg(message) = server_message {
            if message.is_action {
                return;
            }
            let is_mention = message.message_text.to_lowercase().contains(&self.you);
            for reply in Reply::all(&message.channel_login, &self.db_pool).await.unwrap() {
                if is_mention != reply.to_mention {
                    continue;
                }
                match RegexBuilder::new(&reply.pattern)
                    .case_insensitive(reply.case_insensitive)
                    .build()
                {
                    Ok(re) if re.is_match(&message.message_text) => {
                        let reply_expansion =
                            if let Some(channel) = Channel::get(&message.channel_login, &self.db_pool).await.unwrap() {
                                let mut expansion = reply.expansion.replace("`CASTER`", &channel.caster).replace(
                                    "`NOW`",
                                    Utc::now()
                                        .with_timezone(&channel.timezone)
                                        .format("%I:%M %p")
                                        .to_string()
                                        .as_str(),
                                );
                                if let Some(emotes_7tv_id) = channel.emotes_7tv_id {
                                    expansion = expansion.replace("7TV_CHANNEL_ID", &emotes_7tv_id);
                                }
                                expansion
                            } else {
                                reply.expansion
                            };

                        self.irc_client.say_in_reply_to(message, reply_expansion).await.unwrap();
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        dbg!("Invalid pattern for reply {:?} error: {:?}", &reply, e);
                        continue;
                    }
                }
            }
        }
    }
}

