pub mod gamba_time;
pub mod gg;
pub mod npc;
pub mod persistence;
pub mod rendering;
pub mod rip_bozo;
pub mod sniffa;
pub mod the_grind;

pub trait TwitchApiClient: twitch_api::HttpClient + twitch_api::twitch_oauth2::client::Client {}

impl<T: twitch_api::HttpClient + twitch_api::twitch_oauth2::client::Client> TwitchApiClient for T {}

pub trait TwitchApiError: std::error::Error + Send + Sync + 'static {}

impl<T: std::error::Error + Send + Sync + 'static> TwitchApiError for T {}

#[derive(thiserror::Error, Debug)]
pub enum HandlerError<T: twitch_irc::transport::Transport, L: twitch_irc::login::LoginCredentials, RE: TwitchApiError> {
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
pub enum TwitchError<T: twitch_irc::transport::Transport, L: twitch_irc::login::LoginCredentials, RE: TwitchApiError> {
    #[error(transparent)]
    Irc(#[from] twitch_irc::Error<T, L>),
    #[error(transparent)]
    Api(#[from] twitch_api::helix::ClientRequestError<RE>),
}
