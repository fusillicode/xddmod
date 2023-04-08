use std::error::Error;
use std::str::FromStr;

use anyhow::anyhow;
use chrono_tz::Tz;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use sqlx::database::HasValueRef;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::NaiveDate;
use sqlx::types::chrono::Utc;
use sqlx::Database;
use sqlx::Decode;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Channel {
    pub name: String,
    pub caster: String,
    pub date_of_birth: Option<NaiveDate>,
    pub timezone: Timezone,
    pub seven_tv_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Channel {
    pub async fn get<'a>(name: &str, executor: impl SqliteExecutor<'a>) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
                select
                    name as "name!",
                    caster as "caster!",
                    date_of_birth as "date_of_birth: NaiveDate",
                    timezone as "timezone!: Timezone",
                    seven_tv_id,
                    created_at as "created_at!: DateTime<Utc>",
                    updated_at as "updated_at!: DateTime<Utc>"
                from channels
                where name = $1
            "#,
            name
        )
        .fetch_optional(executor)
        .await
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timezone(Tz);

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
        serializer.serialize_str(&self.0.to_string())
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
    use sqlx::types::chrono::TimeZone;
    use sqlx::SqlitePool;

    use super::*;

    #[test]
    fn test_timezone_serde_round_trip() {
        let timezone = Timezone(Tz::Europe__Berlin);

        let ser = serde_json::to_value(&timezone).unwrap();
        assert_eq!("Europe/Berlin", ser.as_str().unwrap());

        let de = serde_json::from_value::<Timezone>(ser).unwrap();
        assert_eq!(de, timezone);
    }

    #[sqlx::test(migrations = "../../migrations")]
    fn test_channel_get(pool: SqlitePool) {
        sqlx::query!(
            r#"
                insert into channels (name, caster, date_of_birth, timezone, seven_tv_id, created_at, updated_at) values
                ("foo", "Foo", "2012-06-06", "CET", "SEVEN_TV_ID_1", "2023-04-07T14:42:43", "2023-04-08T14:42:43"),
                ("bar", "Bar", "2012-07-07", "Asia/Tokyo", "SEVEN_TV_ID_2", "2023-03-06T11:41:42", "2023-04-07T12:40:47");
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            Channel {
                name: "bar".into(),
                caster: "Bar".into(),
                date_of_birth: Some(NaiveDate::from_ymd_opt(2012, 7, 7).unwrap()),
                timezone: Timezone(Tz::Asia__Tokyo),
                seven_tv_id: Some("SEVEN_TV_ID_2".into()),
                created_at: Utc.with_ymd_and_hms(2023, 3, 6, 11, 41, 42).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2023, 4, 7, 12, 40, 47).unwrap(),
            },
            Channel::get("bar", &pool).await.unwrap().unwrap()
        );
    }
}
