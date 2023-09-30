use regex::RegexBuilder;
use serde::Deserialize;
use serde::Serialize;
use sqlx::sqlite::SqliteExecutor;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;
use sqlx::types::Json;
use twitch_irc::message::PrivmsgMessage;
use vec1::Vec1;

#[derive(thiserror::Error, Debug)]
pub enum PersistenceError {
    #[error("single reply {reply:?} matching message {message:?}, regex errors {regex_errors:?}")]
    SingleReplyAndErrors {
        reply: Reply,
        message: String,
        regex_errors: Vec1<regex::Error>,
    },
    #[error("no reply matching message {message:?}, regex errors {regex_errors:?}")]
    NoReplyAndErrors {
        message: String,
        regex_errors: Vec1<regex::Error>,
    },
    #[error("multiple replies {replies:?} matching message {message:?}, regex errors {regex_errors:?}")]
    MultipleReplies {
        replies: Vec<Reply>,
        message: String,
        regex_errors: Vec<regex::Error>,
    },
    #[error("missing additional inputs in {reply:?}")]
    MissingAdditionalInputs { reply: Reply },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
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

impl Reply {
    pub async fn first_matching<'a>(
        handler: Handler,
        matchable_message: &impl MatchableMessage,
        executor: impl SqliteExecutor<'a>,
    ) -> Result<Option<FirstMatching>, PersistenceError> {
        let (replies, regex_errors) = Self::matching_2(handler, matchable_message, executor).await?;

        let message = matchable_message.text();

        match (replies.as_slice(), regex_errors.as_slice()) {
            ([], []) => Ok(None),
            ([reply], regex_errors) => Ok(Some(FirstMatching {
                reply: reply.to_owned(),
                message: message.to_owned(),
                regex_errors: regex_errors.to_owned(),
            })),
            ([], regex_errors) => Err(PersistenceError::NoReplyAndErrors {
                message: message.into(),
                regex_errors: regex_errors.try_into().unwrap(),
            }),
            (replies, regex_errors) => Err(PersistenceError::MultipleReplies {
                replies: replies.to_owned(),
                message: message.into(),
                regex_errors: regex_errors.try_into().unwrap(),
            }),
        }
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

pub struct FirstMatching {
    pub reply: Reply,
    message: String,
    regex_errors: Vec<regex::Error>,
}

impl TryFrom<FirstMatching> for PersistenceError {
    type Error = vec1::Size0Error;

    fn try_from(value: FirstMatching) -> Result<Self, Self::Error> {
        Ok(Self::SingleReplyAndErrors {
            reply: value.reply,
            message: value.message,
            regex_errors: value.regex_errors.try_into()?,
        })
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
