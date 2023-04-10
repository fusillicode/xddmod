use std::error::Error;
use std::str::FromStr;

use anyhow::anyhow;
use chrono_tz::Tz;
use minijinja::context;
use minijinja::Environment;
use regex::RegexBuilder;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use sqlx::database::HasValueRef;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;
use sqlx::types::Json;
use sqlx::Database;
use sqlx::Decode;

#[derive(Debug, Clone)]
pub struct NpcReply {
    pub id: i64,
    pub pattern: String,
    pub case_insensitive: bool,
    pub template: String,
    pub context: Option<Json<Context>>,
    pub to_mention: bool,
    pub channel: Option<String>,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NpcReply {
    pub async fn matching<'a>(
        you: &str,
        channel: &str,
        message_text: &str,
        executor: impl SqliteExecutor<'a>,
    ) -> Vec<NpcReply> {
        let is_mention = message_text.to_lowercase().contains(you);

        Self::all(channel, executor)
            .await
            .unwrap()
            .into_iter()
            .filter(|reply| {
                if is_mention != reply.to_mention {
                    return false;
                }
                match RegexBuilder::new(&reply.pattern)
                    .case_insensitive(reply.case_insensitive)
                    .build()
                {
                    Ok(re) => re.is_match(message_text),
                    Err(e) => {
                        println!("Invalid pattern for reply {:?} error: {:?}", reply, e);
                        false
                    }
                }
            })
            .collect()
    }

    pub fn expand_template(&self) -> Result<String, minijinja::Error> {
        self.context.as_ref().map_or_else(
            || Ok(self.template.to_owned()),
            |c| {
                Environment::new()
                    .render_str::<minijinja::value::Value>(&self.template, minijinja::value::Value::from(&c.0))
            },
        )
    }

    async fn all<'a>(channel: &str, executor: impl SqliteExecutor<'a>) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
                select
                    id,
                    pattern,
                    case_insensitive,
                    template,
                    context as "context: Json<Context>",
                    to_mention,
                    channel,
                    enabled,
                    created_by,
                    created_at as "created_at!: DateTime<Utc>",
                    updated_at as "updated_at!: DateTime<Utc>"
                from npc_replies
                where enabled = 1 and (channel is null or channel = $1)
                order by id asc
            "#,
            channel
        )
        .fetch_all(executor)
        .await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Context {
    Time { timezone: Timezone, format: String },
    Generic(serde_json::Value),
}

impl From<&Context> for minijinja::value::Value {
    fn from(value: &Context) -> Self {
        match value {
            Context::Time { timezone, format } => context! {
                time_in_channel => Utc::now().with_timezone(timezone.as_inner()).format(format).to_string()
            },
            Context::Generic(value) => minijinja::value::Value::from_serializable(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timezone(Tz);

impl Timezone {
    pub fn new(tz: Tz) -> Self {
        Self(tz)
    }
}

impl Timezone {
    pub fn as_inner(&self) -> &Tz {
        &self.0
    }

    pub fn into_inner(self) -> Tz {
        self.0
    }
}

impl FromStr for Timezone {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Tz::from_str(s).map_err(|e| {
            anyhow!("{} cannot be parsed into a chrono_tz::Tz, error: {}", s, e)
        })?))
    }
}

impl Serialize for Timezone {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_inner().to_string())
    }
}

impl<'de> Deserialize<'de> for Timezone {
    fn deserialize<D>(deserializer: D) -> Result<Timezone, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(StringVisitor)
    }
}

struct StringVisitor;

impl<'de> Visitor<'de> for StringVisitor {
    type Value = Timezone;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A valid chrono_tz::Tz string representation")
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Timezone(Tz::from_str(&value).map_err(|_| {
            E::custom(format!("{} cannot be deserialized into a valid chrono_tz::Tz", value))
        })?))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_string(value.to_owned())
    }
}

impl<'r, DB: Database> Decode<'r, DB> for Timezone
where
    &'r str: Decode<'r, DB>,
{
    fn decode(value: <DB as HasValueRef<'r>>::ValueRef) -> Result<Timezone, Box<dyn Error + 'static + Send + Sync>> {
        let value = <&str as Decode<DB>>::decode(value)?;

        Ok(value.parse()?)
    }
}

#[cfg(test)]
mod tests {
    use chrono_tz::Tz;

    use super::*;

    #[test]
    fn test_timezone_serde_round_trip() {
        let timezone = Timezone(Tz::Europe__Berlin);

        let ser = serde_json::to_value(&timezone).unwrap();
        assert_eq!("Europe/Berlin", ser.as_str().unwrap());

        let de = serde_json::from_value::<Timezone>(ser).unwrap();
        assert_eq!(de, timezone);
    }

    #[test]
    fn test_context_serde_json_round_trip() {
        let input = Context::Time {
            timezone: Timezone::new(Tz::Europe__Berlin),
            format: "foo".into(),
        };
        let ser = serde_json::to_string_pretty(&input).unwrap();
        assert_eq!("{\n  \"timezone\": \"Europe/Berlin\",\n  \"format\": \"foo\"\n}", ser);
        let de = serde_json::from_str::<Context>(&ser).unwrap();
        assert_eq!(input, de);

        let input = Context::Generic(serde_json::json!({ "foo": "bar", "baz": 42}));
        let ser = serde_json::to_string_pretty(&input).unwrap();
        assert_eq!("{\n  \"baz\": 42,\n  \"foo\": \"bar\"\n}", ser);
        let de = serde_json::from_str::<Context>(&ser).unwrap();
        assert_eq!(input, de);
    }
}
