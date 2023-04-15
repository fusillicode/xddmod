use std::str::FromStr;

use sqlx::types::chrono::Utc;

pub mod auth;
pub mod handlers;
pub use chrono_tz::Tz;

pub fn build_global_templates_env<'a>() -> minijinja::Environment<'a> {
    let mut template_env = minijinja::Environment::new();
    template_env.add_function("format_date_time", format_date_time);

    template_env
}

fn format_date_time(date_time: &str, format: &str, timezone: &str) -> Result<String, minijinja::Error> {
    let timezone = chrono_tz::Tz::from_str(timezone).unwrap();
    let date_time = if date_time == "now" {
        Utc::now()
    } else {
        serde_json::from_str(date_time).unwrap()
    }
    .with_timezone(&timezone);
    Ok(date_time.format(format).to_string())
}
