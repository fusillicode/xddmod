use std::cmp::Ordering;
use std::str::FromStr;

use chrono_tz::Tz;
use minijinja::Environment;
use minijinja::ErrorKind;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::FixedOffset;
use sqlx::types::chrono::Utc;

pub fn build_global_templates_env<'a>() -> Environment<'a> {
    let mut template_env = Environment::new();
    template_env.add_filter("now", now);
    template_env.add_filter("format_date_time", format_date_time);
    template_env.add_filter("sub_date_times", sub_date_times);
    template_env.add_filter("format_duration", format_duration);
    template_env.add_filter("wrap_string", wrap_string);

    template_env
}

fn now(timezone: Option<&str>) -> Result<String, minijinja::Error> {
    let timezone = timezone.map(parse_timezone).unwrap_or_else(|| Ok(Tz::UTC))?;
    Ok(Utc::now().with_timezone(&timezone).to_rfc3339())
}

fn format_date_time(date_time: &str, format: &str) -> Result<String, minijinja::Error> {
    let date_time = parse_date_time_from_rfc3339(date_time)?;
    Ok(date_time.format(format).to_string())
}

fn sub_date_times(from_date_time: &str, to_date_time: &str) -> Result<minijinja::value::Value, minijinja::Error> {
    let from_date_time = parse_date_time_from_rfc3339(from_date_time)?;
    let to_date_time = parse_date_time_from_rfc3339(to_date_time)?;

    let time_span = match from_date_time.cmp(&to_date_time) {
        Ordering::Less => TimeSpan::InTheFuture {
            duration: (to_date_time - from_date_time).to_std().unwrap(),
        },
        Ordering::Greater => TimeSpan::InThePast {
            duration: (from_date_time - to_date_time).to_std().unwrap(),
        },
        Ordering::Equal => TimeSpan::Zero {
            duration: std::time::Duration::ZERO,
        },
    };

    Ok(minijinja::value::Value::from_serializable(&time_span))
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
enum TimeSpan {
    InTheFuture { duration: std::time::Duration },
    InThePast { duration: std::time::Duration },
    Zero { duration: std::time::Duration },
}

fn format_duration(time_span: &minijinja::value::Value) -> Result<String, minijinja::Error> {
    let duration: std::time::Duration = from_json_value(to_json_value(time_span)?)?;

    let mut formatter = timeago::Formatter::new();
    formatter.ago("");
    formatter.too_low("0");
    formatter.num_items(3);

    Ok(formatter.convert(duration))
}

fn wrap_string(string: &str, wrapping: &str) -> Result<String, minijinja::Error> {
    Ok(format!("{}{}{}", wrapping, string, wrapping))
}

fn parse_timezone(timezone: &str) -> Result<Tz, minijinja::Error> {
    Tz::from_str(timezone).map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Cannot parse &str {:?} as Tz, error {:?}.", timezone, e),
        )
    })
}

fn parse_date_time_from_rfc3339(date_time: &str) -> Result<DateTime<FixedOffset>, minijinja::Error> {
    DateTime::parse_from_rfc3339(date_time).map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Cannot parse {:?} as DateTime, error {:?}.", date_time, e),
        )
        .with_source(e)
    })
}

fn to_json_value(value: &minijinja::value::Value) -> Result<serde_json::Value, minijinja::Error> {
    serde_json::to_value(value).map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "Cannot deserialize &minijinja::Value {:?} into serde_json::Value, error {:?}.",
                value, e
            ),
        )
    })
}

fn from_json_value<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, minijinja::Error> {
    serde_json::from_value(value.clone()).map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "Cannot deserialize serde_json::Value {:?} into T, error {:?}.",
                value, e
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use minijinja::context;
    use serde_json::json;

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
    fn test_sub_date_times_works_as_expected() {
        let past = "2023-04-16T07:52:13.735001+02:00";
        let future = "2023-04-17T07:50:13.739001+02:00";

        let result = serde_json::to_value(sub_date_times(past, future).unwrap()).unwrap();
        assert_eq!(
            result,
            json!({"kind": "InTheFuture", "duration": { "nanos": 4000000, "secs": 86280}})
        );

        let result = serde_json::to_value(sub_date_times(future, past).unwrap()).unwrap();
        assert_eq!(
            result,
            json!({"kind": "InThePast", "duration": { "nanos": 4000000, "secs": 86280}})
        );

        let result = serde_json::to_value(sub_date_times(past, past).unwrap()).unwrap();
        assert_eq!(result, json!({"kind": "Zero", "duration": { "nanos": 0, "secs": 0}}));
    }

    #[test]
    fn test_format_duration_works_as_expected() {
        let template = r#"
            {% if time_span_0.kind == 'InTheFuture' %} still {{ time_span_0.duration | format_duration }} remaning {% endif %}
            {% if time_span_1.kind == 'InThePast' %} {{ time_span_1.duration | format_duration }} ago {% endif %}
            {% if time_span_2.kind == 'Zero' %} {{ time_span_2.duration | format_duration }} ago {% endif %}
        "#;
        let template_context = context! {
                time_span_0 => TimeSpan::InTheFuture { duration: std::time::Duration::new(42999777, 0) },
                time_span_1 => TimeSpan::InThePast { duration: std::time::Duration::new(42999777, 0) },
                time_span_2 => TimeSpan::Zero { duration: std::time::Duration::new(0, 0) },
        };
        let env = build_global_templates_env();

        assert_eq!(
            "\n             still 1 year 4 months 1 week remaning \n             1 year 4 months 1 week ago \n             0 seconds ago \n        ",
            env.render_str(template, template_context).unwrap()
        );
    }

    #[test]
    fn test_wrap_string_as_expected() {
        let template = r#"
            {{ list|map(attribute='title')|map('wrap_string', "'")|join(' vs ') }}
        "#;
        let template_context = context! {
                list => vec![json!({"title": "Foo"}), json!({"title": "Bar"}), json!({"title": "Baz"})],
        };
        let env = build_global_templates_env();

        assert_eq!(
            "\n            'Foo' vs 'Bar' vs 'Baz'\n        ",
            env.render_str(template, template_context).unwrap()
        );
    }
}
