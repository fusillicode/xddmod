use minijinja::value::Value;
use minijinja::Environment;
use sqlx::SqlitePool;
use twitch_irc::login::LoginCredentials;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;
use twitch_irc::transport::Transport;
use twitch_irc::TwitchIRCClient;

use crate::handlers::persistence::Handler;
use crate::handlers::persistence::PersistenceError;
use crate::handlers::persistence::Reply;
use crate::handlers::HandlerError;
use crate::handlers::TwitchApiError;
use crate::handlers::TwitchError;
use crate::poor_man_throttling;

pub struct Npc<'a, T: Transport, L: LoginCredentials> {
    pub irc_client: TwitchIRCClient<T, L>,
    pub db_pool: SqlitePool,
    pub templates_env: Environment<'a>,
}

impl<'a, T: Transport, L: LoginCredentials> Npc<'a, T, L> {
    pub fn handler(&self) -> Handler {
        Handler::Npc
    }
}

impl<'a, T: Transport, L: LoginCredentials> Npc<'a, T, L> {
    pub async fn handle<RE: TwitchApiError>(
        &self,
        server_message: &ServerMessage,
    ) -> Result<(), HandlerError<T, L, RE>> {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            let Some(first_matching) = Reply::first_matching(self.handler(), message, &self.db_pool).await? else {
                return Ok(());
            };

            // FIXME: poor man throttling
            if poor_man_throttling::should_throttle(message, &first_matching.reply)? {
                return Ok(());
            }

            let rendered_reply = first_matching
                .reply
                .render_template::<Value>(&self.templates_env, None)?;

            self.irc_client
                .say_in_reply_to(message, rendered_reply)
                .await
                .map_err(TwitchError::from)?;

            if let Ok(error) = PersistenceError::try_from(first_matching) {
                return Err(error.into());
            };
        }
        Ok(())
    }
}
