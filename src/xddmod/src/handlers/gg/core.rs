use minijinja::value::Value;
use minijinja::Environment;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::handlers::gg::op_gg_client;
use crate::handlers::gg::op_gg_client::Region;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;

pub struct Gg<'a> {
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl<'a> Gg<'a> {
    pub fn handler(&self) -> Handler {
        Handler::Gg
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
                [reply] => {
                    let additional_inputs: AdditionalInputs =
                        serde_json::from_value(reply.additonal_inputs.clone().unwrap().0).unwrap();

                    let summoner =
                        op_gg_client::get_summoner(additional_inputs.region, &additional_inputs.summoner_name)
                            .await
                            .unwrap();
                    let games = op_gg_client::get_games(additional_inputs.region, &summoner.summoner_id, None, None)
                        .await
                        .unwrap();

                    match reply.render_template(&self.templates_env, Some(&Value::from_serializable(&games))) {
                        Ok(rendered_reply) if rendered_reply.is_empty() => {
                            eprintln!("Rendered reply template empty: {:?}.", reply)
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdditionalInputs {
    pub region: Region,
    pub summoner_name: String,
}
