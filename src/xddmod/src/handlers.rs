use twitch_irc::login::LoginCredentials;
use twitch_irc::transport::Transport;

pub mod gamba_time;
pub mod gg;
pub mod npc;
pub mod persistence;
pub mod rendering;
pub mod rip_bozo;
pub mod sniffa;
pub mod the_grind;

#[derive(thiserror::Error, Debug)]
pub enum HandlerError<T: Transport, L: LoginCredentials, RE: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    Persistence(#[from] persistence::PersistenceError),
    #[error(transparent)]
    Rendering(#[from] rendering::RenderingError),
    #[error(transparent)]
    Twitch(#[from] TwitchError<T, L, RE>),
    #[error(transparent)]
    Generic(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum TwitchError<T: Transport, L: LoginCredentials, RE: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    Irc(#[from] twitch_irc::Error<T, L>),
    #[error(transparent)]
    Api(#[from] twitch_api::helix::ClientRequestError<RE>),
}
