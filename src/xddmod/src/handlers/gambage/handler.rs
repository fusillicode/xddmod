use std::time::Duration;

use anyhow::anyhow;
use anyhow::bail;
use minijinja::context;
use minijinja::Environment;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use twitch_api2::helix::predictions::get_predictions::GetPredictionsRequest;
use twitch_api2::helix::predictions::Prediction;
use twitch_api2::twitch_oauth2::UserToken;
use twitch_api2::types::PredictionOutcome;
use twitch_api2::types::PredictionStatus;
use twitch_api2::types::Timestamp;
use twitch_api2::types::UserId;
use twitch_api2::HelixClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;

pub struct Gambage<'a> {
    pub token: UserToken,
    pub broadcaster_id: UserId,
    pub helix_client: HelixClient<'a, reqwest::Client>,
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl<'a> Gambage<'a> {
    pub fn handler(&self) -> Handler {
        Handler::Gamba
    }
}

impl<'a> Gambage<'a> {
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
                    let prediction_request = GetPredictionsRequest::builder()
                        .broadcaster_id(self.broadcaster_id.clone())
                        .first(Some(1))
                        .build();

                    let predictions: Vec<Prediction> = self
                        .helix_client
                        .req_get(prediction_request.clone(), &self.token)
                        .await
                        .unwrap()
                        .data;

                    match predictions.first() {
                        Some(prediction) => match Gamba::try_from(prediction.clone()) {
                            Ok(gamba_data) => {
                                match reply.render_template(&self.templates_env, Some(&context!(gamba => gamba_data))) {
                                    Ok(expaned_reply) if expaned_reply.is_empty() => {
                                        println!("Expanded reply template empty: {:?}.", reply)
                                    }
                                    Ok(expaned_reply) => {
                                        self.irc_client.say_in_reply_to(message, expaned_reply).await.unwrap()
                                    }
                                    Err(e) => println!("Error expanding reply template, error: {:?}, {:?}.", reply, e),
                                }
                            }
                            Err(e) => println!(
                                "Error building GambaData for Prediction {:?}, error: {:?}.",
                                prediction, e
                            ),
                        },
                        None => println!("No Predictions found for request {:?}.", prediction_request),
                    }
                }
                [] => {}
                multiple_matchin_replies => println!(
                    "Multiple matching replies for message: {:?}, {:?}.",
                    multiple_matchin_replies, server_message
                ),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gamba {
    pub title: String,
    pub sides: Vec<Side>,
    pub state: GambaState,
    pub window: Duration,
    pub started_at: Timestamp,
}

impl TryFrom<Prediction> for Gamba {
    type Error = anyhow::Error;

    fn try_from(x: Prediction) -> Result<Self, Self::Error> {
        Ok(Self {
            title: x.title.clone(),
            sides: x.outcomes.clone().into_iter().map(Side::from).collect(),
            state: GambaState::try_from(x.clone())?,
            window: Duration::from_secs(x.prediction_window as u64),
            started_at: x.created_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Side {
    pub title: String,
    pub users: Option<i64>,
    pub betted_channel_points: Option<i64>,
    pub color: String,
}

impl From<PredictionOutcome> for Side {
    fn from(x: PredictionOutcome) -> Self {
        Self {
            title: x.title,
            users: x.users,
            betted_channel_points: x.channel_points,
            color: x.color,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "name")]
pub enum GambaState {
    Up { closing_at: Timestamp },
    Closed { closed_at: Timestamp },
    Payed { winner: Side, payed_at: Timestamp },
    Refunded { refunded_at: Timestamp },
}

impl TryFrom<Prediction> for GambaState {
    type Error = anyhow::Error;

    fn try_from(x: Prediction) -> Result<Self, Self::Error> {
        Ok(match &x.status {
            PredictionStatus::Resolved => {
                let winner_id = x
                    .clone()
                    .winning_outcome_id
                    .ok_or_else(|| anyhow!("Missing winner_outcome_id in {:?} prediction {:?}.", x.status, x))?
                    .to_string();

                let winner = x
                    .outcomes
                    .clone()
                    .into_iter()
                    .find(|o| o.id == winner_id)
                    .map(Side::from)
                    .ok_or_else(|| {
                        anyhow!(
                            "Missing outcome matching winner_id {:?} in {:?} prediction {:?}.",
                            winner_id,
                            x.status,
                            x
                        )
                    })?;

                Self::Payed {
                    winner,
                    payed_at: x
                        .clone()
                        .locked_at
                        .ok_or_else(|| anyhow!("Missing closed_at in {:?} prediction {:?}.", x.status, x))?,
                }
            }
            PredictionStatus::Active => {
                let closing_at = x.created_at.to_fixed_offset() + Duration::from_secs(x.prediction_window as u64);
                Self::Up {
                    closing_at: Timestamp::try_from(closing_at).map_err(|e| {
                        anyhow!("Cannot build closing_at from created_at {:?} and prediction_window {:?} for prediction {:?}, error {:?}.", x.created_at, x.prediction_window, x, e)
                    })?,
                }
            }
            PredictionStatus::Canceled => Self::Refunded {
                refunded_at: x
                    .clone()
                    .locked_at
                    .ok_or_else(|| anyhow!("Missing locked_at in {:?} prediction {:?}.", x.status, x))?,
            },
            PredictionStatus::Locked => Self::Closed {
                closed_at: x
                    .clone()
                    .ended_at
                    .ok_or_else(|| anyhow!("Missing ended_at in {:?} prediction {:?}.", x.status, x))?,
            },
            unexpected_variant => bail!("Unexpected variant {:?} for prediction {:?}.", unexpected_variant, x),
        })
    }
}

#[cfg(test)]
mod tests {
    use sqlx::types::chrono::DateTime;

    use super::*;
    use crate::templates_env::build_global_templates_env;

    #[test]
    fn ciccio() {
        // GAMBA "title" "side" vs "side" is UP since "time ago", you've "seconds" left to bet!
        // GAMBA "title" "side" vs "side" has been closed "time ago" and has been open for "seconds"
        // GAMBA "title" "side" vs "side" resulted in "side" "time ago"
        // GAMBA "title" "side" vs "side" has been refunded "time ago"
        // let template = r#"
        //     GAMBA {{ gamba.title }} {{ gamba.sides|map(attribute='title')|join(' vs ') }}
        //     {% if gamba.state.name == "Up" %}
        //         is UP since {{ format_duration_till_now(gamba.started_at) }}, you've {{ seconds }} left to bet!
        //     {% elif gamba.state.name == "Closed" %}
        //         has been closed {{ format_duration_till_now(gamba.closed_at) }} and has been up for {{ gamba.window
        // }}     {% elif gamba.state.name == "Payed" %}
        //         resulted in {{ gamba.state.winner.title }} {{ format_duration_till_now(gamba.payed_at) }}
        //     {% elif gamba.state.name == "Refunded" %}
        //         has been refunded {{ format_duration_till_now(gamba.refunded_at) }}
        //     {% endif %}
        // "#;
        let template = "{{ ciccio(window) }}";

        let input = Gamba {
            title: "foo".into(),
            sides: vec![
                Side {
                    title: "bar".into(),
                    users: Some(42),
                    betted_channel_points: Some(43),
                    color: "BLUE".into(),
                },
                Side {
                    title: "baz".into(),
                    users: Some(44),
                    betted_channel_points: Some(45),
                    color: "PINK".into(),
                },
            ],
            state: GambaState::Payed {
                winner: Side {
                    title: "bar".into(),
                    users: Some(42),
                    betted_channel_points: Some(43),
                    color: "BLUE".into(),
                },
                payed_at: Timestamp::now(),
            },
            window: Duration::from_secs(32),
            started_at: Timestamp::now(),
        };

        let foo = minijinja::value::Value::from_serializable(&input);
        dbg!(&foo);

        let mut env = build_global_templates_env();
        env.add_function("ciccio", ciccio);
        dbg!(env.render_str(template, foo).unwrap());
    }
}
