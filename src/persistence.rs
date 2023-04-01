use std::str::FromStr;

use chrono_tz::Tz;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;

#[derive(Debug)]
pub struct Reply {
    pub id: i64,
    pub pattern: String,
    pub case_insensitive: bool,
    pub expansion: String,
    pub to_mention: bool,
    pub channel: Option<String>,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Reply {
    pub async fn all<'a>(channel: &str, executor: impl SqliteExecutor<'a>) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
                select
                    id,
                    pattern,
                    case_insensitive,
                    expansion,
                    to_mention,
                    channel,
                    enabled,
                    created_by,
                    created_at as "created_at!: DateTime<Utc>",
                    updated_at as "updated_at!: DateTime<Utc>"
                from replies
                where enabled = 1 and (channel is null or channel = $1)
                order by id desc
            "#,
            channel
        )
        .fetch_all(executor)
        .await
    }
}

#[derive(Debug)]
pub struct Channel {
    pub name: String,
    pub caster: String,
    pub timezone: Tz,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Channel {
    pub async fn get<'a>(name: &str, executor: impl SqliteExecutor<'a>) -> Result<Option<Self>, sqlx::Error> {
        Ok(sqlx::query!(
            r#"
                select
                    name as "name!",
                    caster as "caster!",
                    timezone,
                    created_at as "created_at!: DateTime<Utc>",
                    updated_at as "updated_at!: DateTime<Utc>"
                from channels
                where name = $1
            "#,
            name
        )
        .fetch_optional(executor)
        .await?
        .map(|r| Self {
            name: r.name,
            caster: r.caster,
            timezone: Tz::from_str(r.timezone.as_str()).unwrap(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }
}
