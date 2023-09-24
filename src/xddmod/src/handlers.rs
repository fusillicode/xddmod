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
pub enum HandlerError<T: Transport, L: LoginCredentials> {
    #[error(transparent)]
    Persistence(#[from] persistence::PersistenceError),
    #[error(transparent)]
    Rendering(#[from] rendering::RenderingError),
    #[error(transparent)]
    Twitch(#[from] twitch_irc::Error<T, L>),
    #[error(transparent)]
    Generic(#[from] anyhow::Error),
}
