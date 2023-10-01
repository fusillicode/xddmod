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
