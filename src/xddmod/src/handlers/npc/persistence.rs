use minijinja::context;
use minijinja::Environment;
use regex::RegexBuilder;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;

use crate::persistence::Channel;

#[derive(Debug, Clone)]
pub struct NpcReply {
    pub id: i64,
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

    pub fn expand_with(&self, channel: Option<&Channel>) -> Result<String, minijinja::Error> {
        let env = Environment::new();
        let time_in_channel = channel.map(|c| {
            Utc::now()
                .with_timezone(c.timezone.as_inner())
                .format("%I:%M %p")
                .to_string()
        });
        let ctx = context! { channel, time_in_channel };
        dbg!(&ctx);
        env.render_str(&self.template, ctx)
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
