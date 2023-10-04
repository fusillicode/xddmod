use anyhow::anyhow;
use sqlx::types::chrono::Utc;
use twitch_irc::message::PrivmsgMessage;

use crate::apis::twitch;
use crate::handlers::persistence::Reply;

// FIXME: poor man throttling
lazy_static::lazy_static! {
    static ref THROTTLE: std::sync::Mutex<
        std::collections::BTreeMap<i64, sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>
    >  = std::sync::Mutex::new(std::collections::BTreeMap::new());
}

pub fn should_throttle(message: &PrivmsgMessage, reply: &Reply) -> anyhow::Result<bool> {
    if twitch::helpers::is_from_streamer_or_mod(message) {
        return Ok(false);
    }

    let mut throttle = THROTTLE
        .lock()
        .map_err(|error| anyhow!("Cannot get THROTTLE Lock, error: {:?}", error))?;

    let throttling = throttle
        .get(&reply.id)
        .map(|last_reply_date_time| {
            let time_passed = Utc::now() - *last_reply_date_time;
            time_passed < chrono::Duration::seconds(20)
        })
        .unwrap_or_default();

    if !throttling {
        throttle.insert(reply.id, Utc::now());
    }

    Ok(throttling)
}
