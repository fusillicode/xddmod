use std::time::Duration;

use anyhow::anyhow;
use anyhow::bail;
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
use crate::handlers::persistence::Context;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;

pub struct Gamba<'a> {
    pub token: UserToken,
    pub broadcaster_id: UserId,
    pub helix_client: HelixClient<'a, reqwest::Client>,
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
}

impl<'a> Gamba<'a> {
    pub fn handler(&self) -> Handler {
        Handler::Gamba
    }
}

impl<'a> Gamba<'a> {
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
                        Some(prediction) => match GambaContext::try_from(prediction.clone()) {
                            Ok(gamba_context) => match reply.expand_template(Some(Context::Gamba(gamba_context))) {
                                Ok(expaned_reply) if expaned_reply.is_empty() => {
                                    println!("Expanded reply template empty: {:?}", reply)
                                }
                                Ok(expaned_reply) => {
                                    self.irc_client.say_in_reply_to(message, expaned_reply).await.unwrap()
                                }
                                Err(e) => println!("Error expanding reply template, error: {:?}, {:?}.", reply, e),
                            },
                            Err(e) => println!(
                                "Error building GambaContext from Prediction {:?}, error: {:?}.",
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
// GAMBA "title" "side" vs "side" is UP. It started "time ago", you've "seconds" left to bet!
// GAMBA "title" "side" vs "side" is close Sadge Has been open for "seconds" from "DD/MM HH:MM:SS (UTC)" to "DD/MM
// HH:MM:SS (UTC)" GAMBA "title" "side" vs "side" resulted in "side" win. Has been open for "seconds" from "DD/MM
// HH:MM:SS (UTC)" to "DD/MM HH:MM:SS (UTC)" GAMBA "title" "side" vs "side" has been refunded. Has been open for
// "seconds" from "DD/MM HH:MM:SS (UTC)" to "DD/MM HH:MM:SS (UTC)"
pub struct GambaContext {
    pub title: String,
    pub sides: Vec<Side>,
    pub state: GambaState,
    pub window: Duration,
    pub started_at: Timestamp,
}

impl TryFrom<Prediction> for GambaContext {
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
#[serde(tag = "name")]
pub enum GambaState {
    Up { expected_closed_at: Timestamp },
    Closed { closed_at: Timestamp },
    Payed { winner: Side, closed_at: Timestamp },
    Refunded { refunded_at: Timestamp },
}

impl TryFrom<Prediction> for GambaState {
    type Error = anyhow::Error;

    fn try_from(x: Prediction) -> Result<Self, Self::Error> {
        Ok(match x.status {
            PredictionStatus::Resolved => {
                let winner_id = x
                    .clone()
                    .winning_outcome_id
                    .ok_or_else(|| anyhow!("No winner_outcome_id in Resolved prediction {:?}.", x))?
                    .to_string();

                let winner = x
                    .outcomes
                    .clone()
                    .into_iter()
                    .find(|o| o.id == winner_id)
                    .map(Side::from)
                    .ok_or_else(|| {
                        anyhow!(
                            "No outcome matching winner_id {:?} in prediction outcomes {:?}.",
                            winner_id,
                            x
                        )
                    })?;

                Self::Payed {
                    winner,
                    closed_at: x.locked_at.ok_or_else(|| anyhow!("FOO"))?,
                }
            }
            PredictionStatus::Active => {
                let expected_closed_at =
                    x.created_at.to_fixed_offset() + Duration::from_secs(x.prediction_window as u64);
                Self::Up {
                    expected_closed_at: Timestamp::try_from(expected_closed_at).unwrap(),
                }
            }
            PredictionStatus::Canceled => Self::Refunded {
                refunded_at: x.locked_at.ok_or_else(|| anyhow!("FOO"))?,
            },
            PredictionStatus::Locked => Self::Closed {
                closed_at: x
                    .clone()
                    .ended_at
                    .ok_or_else(|| anyhow!("Missing ended_at in Locked prediction {:?}.", x))?,
            },
            unexpected_variant => bail!("Unexpected PredictionStatus variant {:?}.", unexpected_variant),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name() {
        let input = GambaContext {
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
            },
            window: Duration::from_secs(32),
            started_at: Timestamp::now(),
        };

        dbg!(minijinja::value::Value::from_serializable(&input));
        panic!("asd");
    }
}
