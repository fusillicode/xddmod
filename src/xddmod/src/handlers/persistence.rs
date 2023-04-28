use minijinja::context;
use minijinja::Environment;
use regex::RegexBuilder;
use serde::Deserialize;
use serde::Serialize;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;

#[derive(Debug, Clone)]
pub struct Reply {
    pub id: i64,
    pub handler: Option<Handler>,
    pub pattern: String,
    pub case_insensitive: bool,
    pub template: String,
    pub to_mention: bool,
    pub channel: Option<String>,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Reply {
    pub async fn matching<'a>(
        handler: Handler,
        you: Option<&str>,
        channel: &str,
        message_text: &str,
        executor: impl SqliteExecutor<'a>,
    ) -> Vec<Reply> {
        let is_mention = you.map(|y| message_text.to_lowercase().contains(y));

        Self::all(handler, channel, executor)
            .await
            .unwrap()
            .into_iter()
            .filter(|reply| {
                if is_mention.map(|x| x != reply.to_mention).unwrap_or(false) {
                    return false;
                }
                match RegexBuilder::new(&reply.pattern)
                    .case_insensitive(reply.case_insensitive)
                    .build()
                {
                    Ok(re) => re.is_match(message_text),
                    Err(e) => {
                        eprintln!("Invalid pattern for reply {:?} error: {:?}", reply, e);
                        false
                    }
                }
            })
            .collect()
    }

    pub fn render_template<S: Serialize>(
        &self,
        template_env: &Environment,
        ctx: Option<&S>,
    ) -> Result<String, minijinja::Error> {
        let ctx = match ctx {
            Some(ctx) => minijinja::value::Value::from_serializable(ctx),
            None => context!(),
        };
        template_env.render_str(&self.template, ctx)
    }

    async fn all<'a>(
        handler: Handler,
        channel: &str,
        executor: impl SqliteExecutor<'a>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
                select
                    id,
                    handler as "handler: Handler",
                    pattern,
                    case_insensitive,
                    template,
                    to_mention,
                    channel,
                    enabled,
                    created_by,
                    created_at as "created_at!: DateTime<Utc>",
                    updated_at as "updated_at!: DateTime<Utc>"
                from replies
                where enabled = 1 and (channel is null or channel = $1) and (handler is null or handler = $2)
                order by id asc
            "#,
            channel,
            handler as _,
        )
        .fetch_all(executor)
        .await
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum Handler {
    Npc,
    Gamba,
    Gg,
}
