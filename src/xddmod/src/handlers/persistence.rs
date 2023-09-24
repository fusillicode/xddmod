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

#[derive(thiserror::Error, Debug)]
pub enum PersistenceError {
    #[error("No reply matching message {message:?}, regex errors {regex_errors:?}")]
    NoMatchingReply {
        message: String,
        regex_errors: Vec<regex::Error>,
    },
    #[error("Multiple replies matching message {message:?}, regex errors {regex_errors:?}")]
    MultipleMatchingReplies {
        message: String,
        regex_errors: Vec<regex::Error>,
    },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum RenderingError {
    #[error("Empty rendered reply {reply:?} with template env {template_env:?}")]
    Empty { reply: Reply, template_env: String },
    #[error(transparent)]
    Templating(#[from] minijinja::Error),
}

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
            .map(|x| {
                self.message_text
                    .trim_start_matches(&format!("@{}", x.reply_parent_user.name.as_str()))
                    .trim_start()
            })
            .unwrap_or(&self.message_text)
    }
}

impl Reply {
    pub async fn first_matching<'a>(
        handler: Handler,
        matchable_message: &impl MatchableMessage,
        executor: impl SqliteExecutor<'a>,
    ) -> Result<(Reply, Vec<regex::Error>), PersistenceError> {
        let (replies, regex_errors) = Self::matching_2(handler, matchable_message, executor).await?;

        if let [reply] = replies.as_slice() {
            return Ok((reply.clone(), regex_errors));
        }

        let message = matchable_message.text();

        if replies.first().is_none() {
            return Err(PersistenceError::NoMatchingReply {
                message: message.into(),
                regex_errors,
            });
        }

        Err(PersistenceError::MultipleMatchingReplies {
            message: message.into(),
            regex_errors,
        })
    }

    pub async fn matching_2<'a>(
        handler: Handler,
        matchable_message: &impl MatchableMessage,
        executor: impl SqliteExecutor<'a>,
    ) -> Result<(Vec<Reply>, Vec<regex::Error>), sqlx::Error> {
        let matchable_message_text = matchable_message.text();

        let (matching_replies, re_errors) = Self::all(handler, matchable_message.channel(), executor)
            .await?
            .into_iter()
            .fold((vec![], vec![]), |(mut matching_replies, mut re_errors), reply| {
                match RegexBuilder::new(&reply.pattern)
                    .case_insensitive(reply.case_insensitive)
                    .build()
                {
                    Ok(re) if re.is_match(matchable_message_text) => matching_replies.push(reply),
                    Ok(_) => (),
                    Err(re_error) => re_errors.push(re_error),
                };

                (matching_replies, re_errors)
            });

        Ok((matching_replies, re_errors))
    }

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
    ) -> Result<String, RenderingError> {
        let ctx = ctx.map_or_else(|| context!(), |ctx| minijinja::value::Value::from_serializable(ctx));
        let rendered_reply: String = template_env.render_str(&self.template, ctx).map(|s| s.trim().into())?;

        if rendered_reply.is_empty() {
            return Err(RenderingError::Empty {
                reply: self.clone(),
                template_env: format!("{:?}", template_env),
            });
        }

        Ok(rendered_reply)
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
    TheGrind,
}

#[cfg(test)]
mod tests {
    use fake::Fake;
    use fake::Faker;
    use twitch_irc::message::IRCMessage;
    use twitch_irc::message::IRCTags;
    use twitch_irc::message::ReplyParent;
    use twitch_irc::message::TwitchUserBasics;

    use super::*;

    #[test]
    fn matchable_message_text_works_as_expected() {
        assert_eq!("@foo bar", dummy_privmsg_message("@foo bar".into(), None).text());
        assert_eq!(
            "bar",
            dummy_privmsg_message("@foo bar".into(), Some("foo".into())).text()
        );
        assert_eq!(
            "@foo bar",
            dummy_privmsg_message("@foo bar".into(), Some("baz".into())).text()
        )
    }

    fn dummy_privmsg_message(message_text: String, reply_parent_user_name: Option<String>) -> PrivmsgMessage {
        PrivmsgMessage {
            channel_login: Faker.fake(),
            channel_id: Faker.fake(),
            message_text,
            reply_parent: reply_parent_user_name.map(|name| ReplyParent {
                message_id: Faker.fake(),
                reply_parent_user: TwitchUserBasics {
                    id: Faker.fake(),
                    login: Faker.fake(),
                    name,
                },
                message_text: Faker.fake(),
            }),
            is_action: Faker.fake(),
            sender: TwitchUserBasics {
                id: Faker.fake(),
                login: Faker.fake(),
                name: Faker.fake(),
            },
            badge_info: vec![],
            badges: vec![],
            bits: Faker.fake(),
            name_color: None,
            emotes: vec![],
            message_id: Faker.fake(),
            server_timestamp: Faker.fake(),
            source: IRCMessage {
                tags: IRCTags::new(),
                prefix: None,
                command: Faker.fake(),
                params: Faker.fake(),
            },
        }
    }
}
