use std::str::FromStr;

use chrono_tz::Tz;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::NaiveDate;
use sqlx::types::chrono::Utc;

#[derive(Debug)]
pub struct Channel {
    pub name: String,
    pub caster: String,
    pub date_of_birth: Option<NaiveDate>,
    pub timezone: Tz,
    pub seven_tv_id: Option<String>,
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
                    date_of_birth as "date_of_birth: NaiveDate",
                    timezone,
                    seven_tv_id,
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
            date_of_birth: r.date_of_birth,
            timezone: Tz::from_str(r.timezone.as_str()).unwrap(),
            seven_tv_id: r.seven_tv_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }
}
