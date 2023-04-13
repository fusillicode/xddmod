use sqlx::SqlitePool;
use twitch_api2::helix::predictions::get_predictions::GetPredictionsRequest;
use twitch_api2::helix::predictions::Prediction;
use twitch_api2::twitch_oauth2::UserToken;
use twitch_api2::types::UserId;
use twitch_api2::HelixClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;

use crate::auth::IRCClient;
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
                    let last_gamba_request = GetPredictionsRequest::builder()
                        .broadcaster_id(self.broadcaster_id.clone())
                        .first(Some(1))
                        .build();

                    let gambas: Vec<Prediction> = self
                        .helix_client
                        .req_get(last_gamba_request, &self.token)
                        .await
                        .unwrap()
                        .data;

                    if let Some(last_gamba) = gambas.first() {
                        match reply.expand_template() {
                            Ok(expaned_reply) if expaned_reply.is_empty() => {
                                println!("Expanded reply template empty: {:?}", reply)
                            }
                            Ok(expaned_reply) => self.irc_client.say_in_reply_to(message, expaned_reply).await.unwrap(),
                            Err(e) => println!("Error expanding reply template, error: {:?}, {:?}.", reply, e),
                        }
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
