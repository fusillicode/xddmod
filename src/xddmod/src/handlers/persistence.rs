use minijinja::context;
use minijinja::Environment;
use regex::RegexBuilder;
use serde::Deserialize;
use serde::Serialize;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;
use sqlx::types::Json;
use twitch_irc::message::PrivmsgMessage;

#[derive(Debug, Clone)]
pub struct Reply {
    pub id: i64,
    pub handler: Option<Handler>,
    pub pattern: String,
    pub case_insensitive: bool,
    pub template: String,
    pub channel: Option<String>,
    pub enabled: bool,
    pub additional_inputs: Option<Json<serde_json::Value>>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub trait MatchableMessage {
    fn channel(&self) -> &str;
    fn text(&self) -> &str;
}

impl MatchableMessage for PrivmsgMessage {
    fn channel(&self) -> &str {
        &self.channel_login
    }

    fn text(&self) -> &str {
        self.reply_parent
            .as_ref()
            .map(|x| x.reply_parent_user.name.as_str())
            .map(|x| self.message_text.trim_start_matches(&format!("@{}", x)).trim_start())
            .unwrap_or(&self.message_text)
    }
}

impl Reply {
    pub async fn matching<'a>(
        handler: Handler,
        matchable_message: &impl MatchableMessage,
        executor: impl SqliteExecutor<'a>,
    ) -> Vec<Reply> {
        let matchable_message_text = matchable_message.text();

        Self::all(handler, matchable_message.channel(), executor)
            .await
            .unwrap()
            .into_iter()
            .filter(|reply| {
                match RegexBuilder::new(&reply.pattern)
                    .case_insensitive(reply.case_insensitive)
                    .build()
                {
                    Ok(re) => re.is_match(matchable_message_text),
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
        template_env.render_str(&self.template, ctx).map(|s| s.trim().into())
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
                    channel,
                    enabled,
                    created_by,
                    additional_inputs as "additional_inputs: Json<serde_json::Value>",
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
    Gamba,
    Gg,
    Npc,
    RipBozo,
    Sniffa,
}
