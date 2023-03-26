use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;

pub struct NewCmd {
    pub shortcut: String,
    pub expansion: String,
    pub created_by: String,
}

impl NewCmd {
    pub async fn insert<'a>(&self, executor: impl sqlx::sqlite::SqliteExecutor<'a>) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "insert into cmds (shortcut, expansion, created_by) values ($1, $2, $3)",
            self.shortcut,
            self.expansion,
            self.created_by,
        )
        .execute(executor)
        .await
        .map(|_| ())?;

        Ok(())
    }
}

pub struct Cmd {
    pub id: i64,
    pub shortcut: String,
    pub expansion: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

impl Cmd {
    pub async fn last_by_shortcut<'a>(
        shortcut: &str,
        executor: impl sqlx::sqlite::SqliteExecutor<'a>,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
                select id, shortcut, expansion, created_by, created_at as "created_at!: DateTime<Utc>"
                from cmds
                where shortcut = $1
                order by id desc
            "#,
            shortcut,
        )
        .fetch_optional(executor)
        .await
    }
}
