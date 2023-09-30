use fake::Dummy;
use minijinja::value::Value;
use minijinja::Environment;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use twitch_irc::login::LoginCredentials;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;
use twitch_irc::transport::Transport;
use twitch_irc::TwitchIRCClient;

use crate::apis::ddragon::champion::Champion;
use crate::apis::op_gg;
use crate::apis::op_gg::games::Game;
use crate::apis::op_gg::Region;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::PersistenceError;
use crate::handlers::persistence::Reply;
use crate::handlers::HandlerError;
use crate::handlers::TwitchApiError;
use crate::handlers::TwitchError;
use crate::poor_man_throttling;

pub struct Gg<'a, T: Transport, L: LoginCredentials> {
    pub irc_client: TwitchIRCClient<T, L>,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl<'a, T: Transport, L: LoginCredentials> Gg<'a, T, L> {
    pub fn handler(&self) -> Handler {
        Handler::Gg
    }
}

impl<'a, T: Transport, L: LoginCredentials> Gg<'a, T, L> {
    pub async fn handle<RE: TwitchApiError>(
        &self,
        server_message: &ServerMessage,
    ) -> Result<(), HandlerError<T, L, RE>> {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            let Some(first_matching) = Reply::first_matching(self.handler(), message, &self.db_pool).await? else {
                return Ok(());
            };

            let Some(additional_inputs) = first_matching.reply.additional_inputs.as_ref() else {
                return Err(PersistenceError::MissingAdditionalInputs {
                    reply: first_matching.reply.clone(),
                }
                .into());
            };

            // FIXME: poor man throttling
            if poor_man_throttling::should_throttle(message, &first_matching.reply)? {
                return Ok(());
            }

            match serde_json::from_value::<AdditionalInputs>(additional_inputs.0.clone()) {
                Ok(additional_inputs) => {
                    let summoner =
                        op_gg::summoners::get_summoner(additional_inputs.region, &additional_inputs.summoner_name)
                            .await?;

                    if let Some(game) =
                        op_gg::games::get_last_game(additional_inputs.region, &summoner.common.summoner_id).await?
                    {
                        let template_inputs = TemplateInputs {
                            champion: Champion::by_key(game.my_data.champion_key.into(), &self.db_pool)
                                .await
                                .unwrap(),
                            game,
                        };

                        let rendered_reply = first_matching.reply.render_template::<Value>(
                            &self.templates_env,
                            Some(&Value::from_serializable(&template_inputs)),
                        )?;

                        self.irc_client
                            .say_in_reply_to(message, rendered_reply)
                            .await
                            .map_err(TwitchError::from)?;

                        if let Ok(error) = PersistenceError::try_from(first_matching) {
                            return Err(error.into());
                        };
                    } else {
                        eprintln!("No games returned for reply: {:?}.", first_matching.reply)
                    }
                }
                Err(error) => eprintln!(
                    "Error deserializing AdditionalInputs from Reply for ServerMessage: {:?}, {:?}, {:?}.",
                    error, server_message, first_matching.reply
                ),
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Dummy)]
pub struct AdditionalInputs {
    pub region: Region,
    pub summoner_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Dummy)]
pub struct TemplateInputs {
    pub champion: Option<Champion>,
    pub game: Game,
}
