use std::str::FromStr;

use chrono_tz::Tz;
use minijinja::Environment;
use minijinja::Error;
use minijinja::ErrorKind;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::FixedOffset;
use sqlx::types::chrono::Utc;

pub fn build_global_templates_env<'a>() -> Environment<'a> {
    let mut template_env = Environment::new();
    template_env.add_function("now", now);
    template_env.add_filter("format_date_time", format_date_time);
    template_env.add_function("format_duration", format_duration);

    template_env
}

fn now(timezone: Option<&str>) -> Result<String, Error> {
    let timezone = timezone.map(parse_timezone).unwrap_or_else(|| Ok(Tz::UTC))?;
    Ok(Utc::now().with_timezone(&timezone).to_rfc3339())
}

fn format_date_time(date_time: &str, format: &str) -> Result<String, Error> {
    let date_time = parse_date_time_from_rfc3339(date_time)?;
    Ok(date_time.format(format).to_string())
}

fn parse_timezone(timezone: &str) -> Result<Tz, Error> {
    Tz::from_str(timezone).map_err(|e| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("Cannot parse &str {:?} as Tz, error {:?}.", timezone, e),
        )
    })
}

fn parse_date_time_from_rfc3339(date_time: &str) -> Result<DateTime<FixedOffset>, Error> {
    DateTime::parse_from_rfc3339(date_time).map_err(|e| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("Cannot parse {:?} as DateTime, error {:?}.", date_time, e),
        )
        .with_source(e)
    })
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    #[test]
    fn test_now_works_as_expected() {
        let result = now(None).unwrap();
        assert_eq!(
            "+00:00",
            DateTime::parse_from_rfc3339(&result).unwrap().timezone().to_string()
        );

        let result = now(Some(Tz::Europe__Berlin.to_string().as_str())).unwrap();
        assert_eq!(
            "+02:00",
            DateTime::parse_from_rfc3339(&result).unwrap().timezone().to_string()
        );
    }

    #[test]
    fn test_format_date_time_works_as_expected() {
        assert_eq!(
            "07:52 AM",
            format_date_time("2023-04-16T07:52:13.735001Z", "%I:%M %p").unwrap()
        );
        assert_eq!(
            "07:52 AM",
            format_date_time("2023-04-16T07:52:13.735001+02:00", "%I:%M %p").unwrap()
        );
        assert_eq!("foo", format_date_time("2023-04-16T07:52:13.735001Z", "foo").unwrap());
        assert!(format_date_time("foo", "%I:%M %p").is_err());
    }

    #[test]
    fn test_format_duration_works_as_expected() {
        let now = Utc::now();
        let past = now - Duration::days(1);
        let future = now + Duration::days(1);

        assert_eq!(
            "1 day",
            format_duration(now.to_rfc3339().as_str(), now.to_rfc3339().as_str()).unwrap()
        );
        assert_eq!(
            "1 day",
            format_duration(past.to_rfc3339().as_str(), now.to_rfc3339().as_str()).unwrap()
        );
        assert_eq!(
            "1 day",
            format_duration(future.to_rfc3339().as_str(), now.to_rfc3339().as_str()).unwrap()
        );
    }
}
