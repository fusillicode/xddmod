use std::time::Duration;

use anyhow::anyhow;
use anyhow::bail;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
use fake::Dummy;
use fake::Fake;
use fake::Faker;
use minijinja::Environment;
use serde::Deserialize;
use serde::Serialize;
use sqlx::SqlitePool;
use twitch_api::helix::predictions::get_predictions::GetPredictionsRequest;
use twitch_api::helix::predictions::Prediction;
use twitch_api::twitch_oauth2::UserToken;
use twitch_api::types::PredictionOutcome;
use twitch_api::types::PredictionStatus;
use twitch_api::types::UserId;
use twitch_api::HelixClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
use crate::handlers::persistence::Handler;
use crate::handlers::persistence::Reply;

pub struct GambaTime<'a> {
    pub token: UserToken,
    pub broadcaster_id: UserId,
    pub helix_client: HelixClient<'a, reqwest::Client>,
    pub irc_client: IRCClient,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl GambaTime<'_> {
    pub fn handler(&self) -> Handler {
        Handler::Gamba
    }
}

impl GambaTime<'_> {
    pub async fn handle(&self, server_message: &ServerMessage) {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            match Reply::matching(self.handler(), message, &self.db_pool).await.as_slice() {
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
                            Ok(gamba) => {
                                match reply.render_template(
                                    &self.templates_env,
                                    Some(&minijinja::value::Value::from_serialize(&gamba)),
                                ) {
                                    Ok(rendered_reply) if rendered_reply.is_empty() => {
                                        eprintln!("Rendered reply template empty: {:?}.", reply)
                                    }
                                    Ok(rendered_reply) => {
                                        self.irc_client.say_in_reply_to(message, rendered_reply).await.unwrap()
                                    }
                                    Err(e) => eprintln!("Error rendering reply template, error: {:?}, {:?}.", reply, e),
                                }
                            }
                            Err(e) => eprintln!(
                                "Error building GambaData for Prediction {:?}, error: {:?}.",
                                prediction, e
                            ),
                        },
                        None => eprintln!("No Predictions found for request {:?}.", prediction_request),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gamba {
    pub title: String,
    pub sides: Vec<Side>,
    pub state: GambaState,
    pub duration: Duration,
    pub started_at: DateTime<Utc>,
}

impl Dummy<Faker> for Gamba {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &Faker, rng: &mut R) -> Self {
        Self {
            title: Faker.fake_with_rng(rng),
            sides: Faker.fake_with_rng(rng),
            state: Faker.fake_with_rng(rng),
            duration: std::time::Duration::new(Faker.fake_with_rng(rng), Faker.fake_with_rng(rng)),
            started_at: Faker.fake_with_rng(rng),
        }
    }
}

impl TryFrom<Prediction> for Gamba {
    type Error = anyhow::Error;

    fn try_from(x: Prediction) -> Result<Self, Self::Error> {
        Ok(Self {
            title: x.title.clone(),
            sides: x.outcomes.clone().into_iter().map(Side::from).collect(),
            state: GambaState::try_from(x.clone())?,
            duration: Duration::from_secs(x.prediction_window as u64),
            started_at: Utc.timestamp_nanos(x.created_at.to_utc().unix_timestamp()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Dummy)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Dummy)]
#[serde(tag = "name")]
pub enum GambaState {
    Up,
    Closed { closed_at: DateTime<Utc> },
    Paid { winner: Side, paid_at: DateTime<Utc> },
    Refunded { refunded_at: DateTime<Utc> },
}

impl TryFrom<Prediction> for GambaState {
    type Error = anyhow::Error;

    fn try_from(x: Prediction) -> Result<Self, Self::Error> {
        Ok(match &x.status {
            PredictionStatus::Active => Self::Up,
            PredictionStatus::Locked => Self::Closed {
                closed_at: x
                    .clone()
                    .ended_at
                    .map(|t| Utc.timestamp_nanos(t.to_utc().unix_timestamp()))
                    .ok_or_else(|| anyhow!("Missing ended_at in {:?} prediction {:?}.", x.status, x))?,
            },
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

                Self::Paid {
                    winner,
                    paid_at: x
                        .clone()
                        .locked_at
                        .map(|t| Utc.timestamp_nanos(t.to_utc().unix_timestamp()))
                        .ok_or_else(|| anyhow!("Missing closed_at in {:?} prediction {:?}.", x.status, x))?,
                }
            }
            PredictionStatus::Canceled => Self::Refunded {
                refunded_at: x
                    .clone()
                    .locked_at
                    .map(|t| Utc.timestamp_nanos(t.to_utc().unix_timestamp()))
                    .ok_or_else(|| anyhow!("Missing locked_at in {:?} prediction {:?}.", x.status, x))?,
            },
            unexpected_variant => bail!("Unexpected variant {:?} for prediction {:?}.", unexpected_variant, x),
        })
    }
}
