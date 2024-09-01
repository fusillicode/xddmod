use async_recursion::async_recursion;
use lazy_static::lazy_static;
use regex::Captures;
use regex::Regex;
use sqlx::SqlitePool;
use twitch_api::twitch_oauth2::TwitchToken;
use twitch_api::twitch_oauth2::UserToken;
use twitch_api::HelixClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::message::ServerMessage;
use twitch_types::UserId;
use unicode_segmentation::UnicodeSegmentation;

use crate::apis::twitch;
use crate::handlers::persistence::Handler;

lazy_static! {
    static ref EMOJI_REGEX: Regex = Regex::new(r"\p{Emoji}").unwrap();
    static ref MENTION_REGEX: Regex = Regex::new(r"(@(\w+))").unwrap();
}

pub struct RipBozo<'a> {
    pub broadcaster_id: UserId,
    pub token: UserToken,
    pub helix_client: HelixClient<'a, reqwest::Client>,
    pub db_pool: SqlitePool,
}

impl<'a> RipBozo<'a> {
    pub fn handler(&self) -> Handler {
        Handler::RipBozo
    }
}

impl<'a> RipBozo<'a> {
    pub async fn handle(&mut self, server_message: &ServerMessage) -> anyhow::Result<bool> {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            if twitch::helpers::is_from_streamer_or_mod(message) {
                return Ok(false);
            }

            let mentions = Mentions::new(&message.message_text);
            let message_without_mentions = mentions
                .as_inner()
                .iter()
                .fold(message.message_text.clone(), |acc, mention| {
                    acc.replace(mention.handle, "")
                });

            let text_stats = TextStats::new(&message_without_mentions);
            if text_stats.should_be_deleted() {
                let _ = self.delete_message_with_token_refresh(message, server_message).await;
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[async_recursion]
    async fn delete_message_with_token_refresh(
        &mut self,
        message: &PrivmsgMessage,
        server_message: &ServerMessage,
    ) -> anyhow::Result<()> {
        match self
            .helix_client
            .delete_chat_message(
                &self.broadcaster_id,
                &self.token.user_id,
                &message.message_id,
                &self.token,
            )
            .await
        {
            Ok(delete_response) => {
                println!(
                    "Message deleted {:?}, delete response {:?}",
                    server_message, delete_response
                );
                Ok(())
            }
            Err(error) => {
                eprintln!("Error deleting message {:?}, error {:?}", server_message, error);

                if twitch::helpers::is_unauthorized_error(&error) {
                    eprintln!("Refreshing token");
                    self.token.refresh_token(self.helix_client.get_client()).await?;
                    return self.delete_message_with_token_refresh(message, server_message).await;
                }

                Err(error.into())
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Mention<'a> {
    handle: &'a str,
    login: &'a str,
}

impl<'a> Mention<'a> {
    pub fn from(captures: Captures<'a>) -> Option<Self> {
        match (captures.get(1), captures.get(2)) {
            (Some(handle), Some(login)) => Some(Self {
                handle: handle.as_str(),
                login: login.as_str(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Mentions<'a>(Vec<Mention<'a>>);

impl<'a> Mentions<'a> {
    pub fn new(text: &'a str) -> Self {
        Self(MENTION_REGEX.captures_iter(text).filter_map(Mention::from).collect())
    }

    pub fn as_inner(&self) -> &[Mention<'_>] {
        self.0.as_slice()
    }
}

#[allow(dead_code)]
struct TextStats<'a> {
    graphemes: Vec<&'a str>,
    ascii_alnum: Vec<char>,
    ascii_symbols: Vec<char>,
    not_ascii: Vec<&'a str>,
    emojis: Vec<&'a str>,
    whitespaces: Vec<&'a str>,
}

impl<'a> TextStats<'a> {
    const NOT_ASCII_WHITELIST: [&'static str; 4] = ["\u{e0000}", "…", "？", "о"];

    pub fn new(text: &'a str) -> Self {
        let graphemes: Vec<&str> = UnicodeSegmentation::graphemes(text, true).collect();

        let mut ascii_alnum = vec![];
        let mut ascii_symbols = vec![];
        let mut not_ascii = vec![];
        let mut emojis = vec![];
        let mut whitespaces = vec![];

        for g in graphemes.iter() {
            match g.is_ascii() {
                _ if g.trim().is_empty() => whitespaces.push(*g),
                true => {
                    for c in g.chars() {
                        if c.is_alphanumeric() {
                            ascii_alnum.push(c)
                        } else {
                            ascii_symbols.push(c)
                        }
                    }
                }
                false if EMOJI_REGEX.is_match(g) => emojis.push(*g),
                false => not_ascii.push(*g),
            }
        }

        Self {
            graphemes,
            ascii_alnum,
            ascii_symbols,
            not_ascii,
            emojis,
            whitespaces,
        }
    }

    pub fn should_be_deleted(&self) -> bool {
        if let Some(emojis_count) = self.only_emojis() {
            return emojis_count > 24;
        }

        if self.not_ascii_perc(&Self::NOT_ASCII_WHITELIST) > 45.0 {
            return true;
        }

        false
    }

    fn only_emojis(&self) -> Option<usize> {
        let emojis_count = self.emojis.len();
        if emojis_count == self.ascii_alnum.len() + self.ascii_symbols.len() + self.not_ascii.len() + self.emojis.len()
        {
            return Some(emojis_count);
        }
        None
    }

    fn not_ascii_perc(&self, not_ascii_whitelist: &[&str]) -> f64 {
        let not_ascii_count = self
            .not_ascii
            .iter()
            .filter(|x| !not_ascii_whitelist.contains(x))
            .count();

        (not_ascii_count as f64 / (not_ascii_count + self.ascii_alnum.len() + self.ascii_symbols.len()) as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mentions_new_builds_the_expected_mentions() {
        assert!(Mentions::new(r#""#).as_inner().is_empty());
        assert_eq!(
            Mentions::new(r#"@Fusillicode"#).as_inner(),
            &[Mention {
                handle: "@Fusillicode",
                login: "Fusillicode"
            }]
        );
        assert_eq!(
            Mentions::new(r#"@Fusilli code"#).as_inner(),
            &[Mention {
                handle: "@Fusilli",
                login: "Fusilli"
            }]
        );
        assert_eq!(
            Mentions::new(r#"@Fusilli @code"#).as_inner(),
            &[
                Mention {
                    handle: "@Fusilli",
                    login: "Fusilli"
                },
                Mention {
                    handle: "@code",
                    login: "code"
                }
            ]
        );
        assert_eq!(
            Mentions::new(r#"@Fusilli@code"#).as_inner(),
            &[
                Mention {
                    handle: "@Fusilli",
                    login: "Fusilli"
                },
                Mention {
                    handle: "@code",
                    login: "code"
                }
            ]
        );
        assert_eq!(
            Mentions::new(r#"@Fusilli, code"#).as_inner(),
            &[Mention {
                handle: "@Fusilli",
                login: "Fusilli"
            }]
        );
        assert_eq!(
            Mentions::new(r#"Fusilli, @code"#).as_inner(),
            &[Mention {
                handle: "@code",
                login: "code"
            }]
        );
    }

    #[test]
    fn test_text_stats_should_delete_works_as_expected() {
        assert!(!TextStats::new(r#""#).should_be_deleted());
        assert!(!TextStats::new(r#" "#).should_be_deleted());
        assert!(!TextStats::new(r#"D:"#).should_be_deleted());
        assert!(!TextStats::new(r#":D"#).should_be_deleted());
        assert!(!TextStats::new(r#":)"#).should_be_deleted());
        assert!(!TextStats::new(r#":) :) :) :) :) :) :) :) :)"#).should_be_deleted());
        assert!(!TextStats::new(r#"hola"#).should_be_deleted());
        assert!(!TextStats::new(r#"..."#).should_be_deleted());
        assert!(!TextStats::new(r#"......"#).should_be_deleted());
        assert!(!TextStats::new(r#"........."#).should_be_deleted());
        assert!(!TextStats::new(r#"!!!"#).should_be_deleted());
        assert!(!TextStats::new(r#"!!!!!!"#).should_be_deleted());
        assert!(!TextStats::new(r#"!!!!!!!!!"#).should_be_deleted());
        assert!(!TextStats::new(r#"???"#).should_be_deleted());
        assert!(!TextStats::new(r#"??????"#).should_be_deleted());
        assert!(!TextStats::new(r#"?????????"#).should_be_deleted());
        assert!(!TextStats::new(r#"WTF!?!?!?!??!?!?!???!?!?!?"#).should_be_deleted());
        assert!(!TextStats::new(r#"@@"#).should_be_deleted());
        assert!(!TextStats::new(r#"…"#).should_be_deleted());
        assert!(!TextStats::new(r#"…o"#).should_be_deleted());
        assert!(!TextStats::new(
            r#""El presidente del Congreso, que aún no ha manifestado si se adherirá o no a la iniciativa del
        ministro de Industria, no quiso dar trascendencia al asunto, «que no tiene más valor que el de una anécdota y
        el de una corbata regalada»."#
        )
        .should_be_deleted());
        assert!(!TextStats::new(r#"🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲"#).should_be_deleted());
        assert!(
            !TextStats::new(r#"🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲"#)
                .should_be_deleted()
        );
        assert!(!TextStats::new(r#"WHAT?!!! 🔥🔥🔥🗣️💯💯💯"#).should_be_deleted());
        assert!(!TextStats::new("🐝 \u{e0000}").should_be_deleted());
        assert!(!TextStats::new("A \u{e0000}").should_be_deleted());
        assert!(!TextStats::new("？").should_be_deleted());
        assert!(!TextStats::new("foo ？").should_be_deleted());
        assert!(!TextStats::new("о").should_be_deleted());
        assert!(!TextStats::new("о7").should_be_deleted());
        assert!(TextStats::new(r#"…ö"#).should_be_deleted());
        assert!(
            TextStats::new(r#"🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲"#)
                .should_be_deleted()
        );
        assert!(TextStats::new(
            r#"🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲
        🥲 🥲 🥲 🥲 🥲 🥲"#
        )
        .should_be_deleted());
        assert!(TextStats::new(
            r#"
                ✅✅✅✅✅✅✅✅✅✅✅✅
                ✅✅✅✅✅✅✅✅✅✅✅✅
                ✅✅⬛⬛⬛✅✅⬛⬛⬛✅✅
                ✅✅⬛⬛⬛✅✅⬛⬛⬛✅✅
                ✅✅✅✅✅⬛⬛✅✅✅✅✅
                ✅✅✅⬛⬛⬛⬛⬛⬛✅✅✅
                ✅✅✅⬛⬛⬛⬛⬛⬛✅✅✅
                ✅✅✅⬛⬛✅✅⬛⬛✅✅✅
                ✅✅✅✅✅✅✅✅✅✅✅✅
            "#
        )
        .should_be_deleted());
        assert!(TextStats::new(
            r#"
                ⢿⣿⣿⣿⣭⠹⠛⠛⠛⢿⣿⣿⣿⣿⡿⣿⠷⠶⠿⢻⣿⣛⣦⣙⠻⣿
                ⣿⣿⢿⣿⠏⠀⠀⡀⠀⠈⣿⢛⣽⣜⠯⣽⠀⠀⠀⠀⠙⢿⣷⣻⡀⢿
                ⠐⠛⢿⣾⣖⣤⡀⠀⢀⡰⠿⢷⣶⣿⡇⠻⣖⣒⣒⣶⣿⣿⡟⢙⣶⣮
                ⣤⠀⠀⠛⠻⠗⠿⠿⣯⡆⣿⣛⣿⡿⠿⠮⡶⠼⠟⠙⠊⠁⠀⠸⢣⣿
                ⣿⣷⡀⠀⠀⠀⠀⠠⠭⣍⡉⢩⣥⡤⠥⣤⡶⣒⠀⠀⠀⠀⠀⢰⣿⣿
                ⣿⣿⡽⡄⠀⠀⠀⢿⣿⣆⣿⣧⢡⣾⣿⡇⣾⣿⡇⠀⠀⠀⠀⣿⡇⠃
                ⣿⣿⣷⣻⣆⢄⠀⠈⠉⠉⠛⠛⠘⠛⠛⠛⠙⠛⠁⠀⠀⠀⠀⣿⡇⢸
                ⢞⣿⣿⣷⣝⣷⣝⠦⡀⠀⠀⠀⠀⠀⠀⠀⡀⢀⠀⠀⠀⠀⠀⠛⣿⠈
                ⣦⡑⠛⣟⢿⡿⣿⣷⣝⢧⡀⠀⠀⣶⣸⡇⣿⢸⣧⠀⠀⠀⠀⢸⡿⡆
                ⣿⣿⣷⣮⣭⣍⡛⠻⢿⣷⠿⣶⣶⣬⣬⣁⣉⣀⣀⣁⡤⢴⣺⣾⣽⡇
            "#
        )
        .should_be_deleted());
        assert!(TextStats::new(
            r#"⢿⣿⣿⣿⣭⠹⠛⠛⠛⢿⣿⣿⣿⣿⡿⣿⠷⠶⠿⢻⣿⣛⣦⣙⠻⣿ ⣿⣿⢿⣿⠏⠀⠀⡀⠀⠈⣿⢛⣽⣜⠯⣽⠀⠀⠀⠀⠙⢿⣷⣻⡀⢿ ⠐⠛⢿⣾⣖⣤⡀⠀⢀⡰⠿⢷⣶⣿⡇⠻⣖⣒⣒⣶⣿⣿⡟⢙⣶⣮ ⣤⠀⠀⠛⠻⠗⠿⠿⣯⡆⣿⣛⣿⡿⠿⠮⡶⠼⠟⠙⠊⠁⠀⠸⢣⣿ ⣿⣷⡀⠀⠀⠀⠀⠠⠭⣍⡉⢩⣥⡤⠥⣤⡶⣒⠀⠀⠀⠀⠀⢰⣿⣿ ⣿⣿⡽⡄⠀⠀⠀⢿⣿⣆⣿⣧⢡⣾⣿⡇⣾⣿⡇⠀⠀⠀⠀⣿⡇⠃ ⣿⣿⣷⣻⣆⢄⠀⠈⠉⠉⠛⠛⠘⠛⠛⠛⠙⠛⠁⠀⠀⠀⠀⣿⡇⢸ ⢞⣿⣿⣷⣝⣷⣝⠦⡀⠀⠀⠀⠀⠀⠀⠀⡀⢀⠀⠀⠀⠀⠀⠛⣿⠈ ⣦⡑⠛⣟⢿⡿⣿⣷⣝⢧⡀⠀⠀⣶⣸⡇⣿⢸⣧⠀⠀⠀⠀⢸⡿⡆ ⣿⣿⣷⣮⣭⣍⡛⠻⢿⣷⠿⣶⣶⣬⣬⣁⣉⣀⣀⣁⡤⢴⣺⣾⣽⡇"#
        ).should_be_deleted());
        assert!(TextStats::new(
            r#"⢿⣿⣿⣿⣭⠹⠛⠛⠛⢿⣿⣿⣿⣿⡿⣿⠷⠶⠿⢻⣿⣛⣦⣙⠻⣿⣿⣿⢿⣿⠏⠀⠀⡀⠀⠈⣿⢛⣽⣜⠯⣽⠀⠀⠀⠀⠙⢿⣷⣻⡀⢿⠐⠛⢿⣾⣖⣤⡀⠀⢀⡰⠿⢷⣶⣿⡇⠻⣖⣒⣒⣶⣿⣿⡟⢙⣶⣮⣤⠀⠀⠛⠻⠗⠿⠿⣯⡆⣿⣛⣿⡿⠿⠮⡶⠼⠟⠙⠊⠁⠀⠸⢣⣿⣿⣷⡀⠀⠀⠀⠀⠠⠭⣍⡉⢩⣥⡤⠥⣤⡶⣒⠀⠀⠀⠀⠀⢰⣿⣿⣿⣿⡽⡄⠀⠀⠀⢿⣿⣆⣿⣧⢡⣾⣿⡇⣾⣿⡇⠀⠀⠀⠀⣿⡇⠃⣿⣿⣷⣻⣆⢄⠀⠈⠉⠉⠛⠛⠘⠛⠛⠛⠙⠛⠁⠀⠀⠀⠀⣿⡇⢸⢞⣿⣿⣷⣝⣷⣝⠦⡀⠀⠀⠀⠀⠀⠀⠀⡀⢀⠀⠀⠀⠀⠀⠛⣿⠈⣦⡑⠛⣟⢿⡿⣿⣷⣝⢧⡀⠀⠀⣶⣸⡇⣿⢸⣧⠀⠀⠀⠀⢸⡿⡆⣿⣿⣷⣮⣭⣍⡛⠻⢿⣷⠿⣶⣶⣬⣬⣁⣉⣀⣀⣁⡤⢴⣺⣾⣽⡇"#
        ).should_be_deleted());
        assert!(TextStats::new(
            r"
                ▬▬▬▬▬.◙.▬▬▬▬▬
                ▂▄▄▓▄▄▂
            ◢◤█▀▀████▄▄▄▄▄▄ ◢◤
            █▄ █ █▄ ███▀▀▀▀▀▀▀ ╬
            ◥ █████ ◤
                ══╩══╩═
                ╬═╬
                ╬═╬ just dropped down to say
                ╬═╬
                ╬═╬ I forgor
                ╬═╬
            💀/ ╬═╬
            /▌  ╬═╬
            / \
            "
        )
        .should_be_deleted());
        assert!(TextStats::new(
            r#"
                ———————————No stitches?———————————
                ⠀⣞⢽⢪⢣⢣⢣⢫⡺⡵⣝⡮⣗⢷⢽⢽⢽⣮⡷⡽⣜⣜⢮⢺⣜⢷⢽⢝⡽⣝
                ⠸⡸⠜⠕⠕⠁⢁⢇⢏⢽⢺⣪⡳⡝⣎⣏⢯⢞⡿⣟⣷⣳⢯⡷⣽⢽⢯⣳⣫⠇
                ⠀⠀⢀⢀⢄⢬⢪⡪⡎⣆⡈⠚⠜⠕⠇⠗⠝⢕⢯⢫⣞⣯⣿⣻⡽⣏⢗⣗⠏⠀
                ⠀⠪⡪⡪⣪⢪⢺⢸⢢⢓⢆⢤⢀⠀⠀⠀⠀⠈⢊⢞⡾⣿⡯⣏⢮⠷⠁⠀⠀
                ⠀⠀⠀⠈⠊⠆⡃⠕⢕⢇⢇⢇⢇⢇⢏⢎⢎⢆⢄⠀⢑⣽⣿⢝⠲⠉⠀⠀⠀⠀
                ⠀⠀⠀⠀⠀⡿⠂⠠⠀⡇⢇⠕⢈⣀⠀⠁⠡⠣⡣⡫⣂⣿⠯⢪⠰⠂⠀⠀⠀⠀
                ⠀⠀⠀⠀⡦⡙⡂⢀⢤⢣⠣⡈⣾⡃⠠⠄⠀⡄⢱⣌⣶⢏⢊⠂⠀⠀⠀⠀⠀⠀
                ⠀⠀⠀⠀⢝⡲⣜⡮⡏⢎⢌⢂⠙⠢⠐⢀⢘⢵⣽⣿⡿⠁⠁⠀⠀⠀⠀⠀⠀⠀
                ⠀⠀⠀⠀⠨⣺⡺⡕⡕⡱⡑⡆⡕⡅⡕⡜⡼⢽⡻⠏⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
                ⠀⠀⠀⠀⣼⣳⣫⣾⣵⣗⡵⡱⡡⢣⢑⢕⢜⢕⡝⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
                ⠀⠀⠀⣴⣿⣾⣿⣿⣿⡿⡽⡑⢌⠪⡢⡣⣣⡟⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
                ⠀⠀⠀⡟⡾⣿⢿⢿⢵⣽⣾⣼⣘⢸⢸⣞⡟⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
                ⠀⠀⠀⠀⠁⠇⠡⠩⡫⢿⣝⡻⡮⣒⢽⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
                ————————————————————————————-
            "#
        )
        .should_be_deleted());
    }
}
