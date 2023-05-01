use fake::Dummy;
use minijinja::value::Value;
use minijinja::Environment;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::apis::op_gg::Region;
use crate::auth::IRCClient;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;

pub struct Sniffa<'a> {
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl<'a> Sniffa<'a> {
    pub fn handler(&self) -> Handler {
        Handler::Sniffa
    }
}

impl<'a> Sniffa<'a> {
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
                [reply @ Reply {
                    additional_inputs: Some(additional_inputs),
                    ..
                }] => match serde_json::from_value::<AdditionalInputs>(additional_inputs.0.clone()) {
                    Ok(additional_inputs) => {
                        match reply
                            .render_template(&self.templates_env, Some(&Value::from_serializable(&additional_inputs)))
                        {
                            Ok(rendered_reply) if rendered_reply.is_empty() => {
                                eprintln!("Rendered reply template empty: {:?}.", reply)
                            }
                            Ok(rendered_reply) => {
                                self.irc_client.say_in_reply_to(message, rendered_reply).await.unwrap()
                            }
                            Err(e) => eprintln!("Error rendering reply template, error: {:?}, {:?}.", reply, e),
                        }
                    }
                    Err(error) => eprintln!(
                        "Error deserializing AdditionalInputs from Reply for ServerMessage: {:?}, {:?}, {:?}.",
                        error, server_message, reply
                    ),
                },
                [reply @ Reply {
                    additional_inputs: None,
                    ..
                }] => eprintln!(
                    "Reply for ServerMessage with missing AdditionalInputs: {:?}, {:?}.",
                    server_message, reply
                ),
                [] => {}
                multiple_matching_replies => eprintln!(
                    "Multiple matching replies for message: {:?}, {:?}.",
                    multiple_matching_replies, server_message
                ),
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Dummy)]
pub struct AdditionalInputs {
    pub region: Region,
    pub summoner_name: String,
}
