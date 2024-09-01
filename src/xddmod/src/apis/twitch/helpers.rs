use http::StatusCode;
use twitch_api::helix::ClientRequestError;
use twitch_api::helix::HelixRequestDeleteError;
use twitch_api::helix::HelixRequestGetError;
use twitch_api::helix::HelixRequestPatchError;
use twitch_api::helix::HelixRequestPostError;
use twitch_api::helix::HelixRequestPutError;
use twitch_irc::message::PrivmsgMessage;

pub fn is_unauthorized_error<T: std::error::Error + Send + Sync + 'static>(error: &ClientRequestError<T>) -> bool {
    matches!(
        error,
        ClientRequestError::HelixRequestGetError(HelixRequestGetError::Error {
            status: StatusCode::UNAUTHORIZED,
            ..
        }) | ClientRequestError::HelixRequestPutError(HelixRequestPutError::Error {
            status: StatusCode::UNAUTHORIZED,
            ..
        }) | ClientRequestError::HelixRequestPostError(HelixRequestPostError::Error {
            status: StatusCode::UNAUTHORIZED,
            ..
        }) | ClientRequestError::HelixRequestPatchError(HelixRequestPatchError::Error {
            status: StatusCode::UNAUTHORIZED,
            ..
        }) | ClientRequestError::HelixRequestDeleteError(HelixRequestDeleteError::Error {
            status: StatusCode::UNAUTHORIZED,
            ..
        })
    )
}

pub fn is_from_streamer_or_mod(message: &PrivmsgMessage) -> bool {
    message
        .badges
        .iter()
        .any(|b| b.name == "moderator" || b.name == "broadcaster")
}
