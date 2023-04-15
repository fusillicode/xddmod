use std::str::FromStr;

use chrono_tz::Tz;
use minijinja::Environment;
use minijinja::Error;
use minijinja::ErrorKind;
use sqlx::types::chrono::Utc;

pub fn build_global_templates_env<'a>() -> Environment<'a> {
    let mut template_env = Environment::new();
    template_env.add_function("format_date_time", format_date_time);

    template_env
}

fn format_date_time(date_time: &str, timezone: &str, format: &str) -> Result<String, Error> {
    let timezone = Tz::from_str(timezone).map_err(|e| {
        Error::new(
            ErrorKind::MissingArgument,
            format!("Cannot deserialize Tz from &str {:?}, error {:?}", timezone, e),
        )
    })?;

    let date_time = if date_time == "now" {
        Utc::now()
    } else {
        serde_json::from_str(date_time).map_err(|e| {
            Error::new(
                ErrorKind::MissingArgument,
                format!(
                    "Cannot deserialize DateTime from JSON string {:?}, error {:?}",
                    date_time, e
                ),
            )
        })?
    }
    .with_timezone(&timezone);

    Ok(date_time.format(format).to_string())
}
