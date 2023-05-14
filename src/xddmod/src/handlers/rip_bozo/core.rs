use async_recursion::async_recursion;
use lazy_static::lazy_static;
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
    pub async fn handle(&mut self, server_message: &ServerMessage) {
        if let ServerMessage::Privmsg(message @ PrivmsgMessage { is_action: false, .. }) = server_message {
            if !twitch::helpers::is_from_streamer_or_mod(message) && should_delete(&message.message_text) {
                self.delete_message_with_token_refresh(message, server_message).await;
            }
        }
    }

    #[async_recursion]
    async fn delete_message_with_token_refresh(&mut self, message: &PrivmsgMessage, server_message: &ServerMessage) {
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
            Ok(delete_response) => println!(
                "Message deleted {:?}, delete response {:?}",
                server_message, delete_response
            ),
            Err(error) => {
                eprintln!("Error deleting message {:?}, error {:?}", server_message, error);

                if twitch::helpers::is_unauthorized_error(&error) {
                    eprintln!("Refreshing token");
                    self.token.refresh_token(self.helix_client.get_client()).await.unwrap();
                    self.delete_message_with_token_refresh(message, server_message).await
                }
            }
        }
    }
}

lazy_static! {
    static ref EMOJI_REGEX: Regex = Regex::new(r#"\p{Emoji}"#).unwrap();
}

struct TextStats<'a> {
    _graphemes: Vec<&'a str>,
    ascii_alnums: Vec<char>,
    ascii_symbols: Vec<char>,
    not_ascii: Vec<&'a str>,
    emojis: Vec<&'a str>,
    whitespaces: Vec<&'a str>,
}

impl<'a> TextStats<'a> {
    pub fn build(text: &'a str) -> Self {
        let graphemes: Vec<&str> = UnicodeSegmentation::graphemes(text, true).collect();

        let mut ascii_alnums = vec![];
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
                            ascii_alnums.push(c)
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
            _graphemes: graphemes,
            ascii_alnums,
            ascii_symbols,
            not_ascii,
            emojis,
            whitespaces,
        }
    }

    pub fn only_emojis(&self) -> Option<usize> {
        let emojis_count = self.emojis.len();
        if emojis_count == self.ascii_alnums.len() + self.ascii_symbols.len() + self.not_ascii.len() + self.emojis.len()
        {
            return Some(emojis_count);
        }
        None
    }

    pub fn not_alnum_perc(&self) -> f64 {
        let not_alunm_count = self.not_ascii.len() + self.ascii_symbols.len();

        (not_alunm_count as f64 / (not_alunm_count + self.ascii_alnums.len()) as f64) * 100.0
    }

    pub fn total_count(&self) -> usize {
        self.ascii_alnums.len()
            + self.ascii_symbols.len()
            + self.not_ascii.len()
            + self.emojis.len()
            + self.whitespaces.len()
    }
}

fn should_delete(message_text: &str) -> bool {
    let text_stats = TextStats::build(message_text);

    if text_stats.total_count() < 24 {
        return false;
    }

    if let Some(emojis_count) = text_stats.only_emojis() {
        return emojis_count > 24;
    }

    text_stats.not_alnum_perc() > 45.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_delete() {
        assert!(!should_delete(r#""#));
        assert!(!should_delete(r#" "#));
        assert!(!should_delete(r#"hola"#));
        assert!(!should_delete(r#"..."#));
        assert!(!should_delete(r#"......"#));
        assert!(!should_delete(r#"........."#));
        assert!(!should_delete(r#"!!!"#));
        assert!(!should_delete(r#"!!!!!!"#));
        assert!(!should_delete(r#"!!!!!!!!!"#));
        assert!(!should_delete(r#"???"#));
        assert!(!should_delete(r#"??????"#));
        assert!(!should_delete(r#"?????????"#));
        assert!(!should_delete(r#"…"#));
        assert!(!should_delete(r#"…o"#));
        assert!(!should_delete(
            r#""El presidente del Congreso, que aún no ha manifestado si se adherirá o no a la iniciativa del ministro de Industria, no quiso dar trascendencia al asunto, «que no tiene más valor que el de una anécdota y el de una corbata regalada»."#
        ));
        assert!(!should_delete(r#"🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲"#));
        assert!(!should_delete(
            r#"🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲"#
        ));
        assert!(!should_delete(r#"WHAT?!!! 🔥🔥🔥🗣️💯💯💯"#));
        assert!(!should_delete("🐝 \u{e0000}"));
        assert!(!should_delete("A \u{e0000}"));
        assert!(!should_delete("？"));
        assert!(!should_delete("foo ？"));
        assert!(!should_delete("о"));
        assert!(!should_delete("о7"));
        assert!(!should_delete(r#"…ö"#));
        assert!(!should_delete(r#"@@"#));
        assert!(should_delete(
            r#"🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲"#
        ));
        assert!(should_delete(
            r#"🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲"#
        ));
        assert!(should_delete(
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
        ));
        assert!(!should_delete(
            r#"
                YOU’VE BEEN FREAKING HIT BY THE

                |^^^^^^^^^^^^](ﾉ◕ヮ◕)ﾉ*:･ﾟ✧
                | KAWAII TRUCK | ‘|”“”;.., ___.
                |_…_…______===|= _|__|…, ] |
                ”(@ )’(@ )”“”“*|(@ )(@ )*****(@　　　　⊂（ﾟДﾟ⊂⌒） NO KAWAII TRUCK NO!!!

                RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM RANDOM.
            "#
        ));
        assert!(should_delete(
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
        ));
        assert!(should_delete(
            r#"⢿⣿⣿⣿⣭⠹⠛⠛⠛⢿⣿⣿⣿⣿⡿⣿⠷⠶⠿⢻⣿⣛⣦⣙⠻⣿ ⣿⣿⢿⣿⠏⠀⠀⡀⠀⠈⣿⢛⣽⣜⠯⣽⠀⠀⠀⠀⠙⢿⣷⣻⡀⢿ ⠐⠛⢿⣾⣖⣤⡀⠀⢀⡰⠿⢷⣶⣿⡇⠻⣖⣒⣒⣶⣿⣿⡟⢙⣶⣮ ⣤⠀⠀⠛⠻⠗⠿⠿⣯⡆⣿⣛⣿⡿⠿⠮⡶⠼⠟⠙⠊⠁⠀⠸⢣⣿ ⣿⣷⡀⠀⠀⠀⠀⠠⠭⣍⡉⢩⣥⡤⠥⣤⡶⣒⠀⠀⠀⠀⠀⢰⣿⣿ ⣿⣿⡽⡄⠀⠀⠀⢿⣿⣆⣿⣧⢡⣾⣿⡇⣾⣿⡇⠀⠀⠀⠀⣿⡇⠃ ⣿⣿⣷⣻⣆⢄⠀⠈⠉⠉⠛⠛⠘⠛⠛⠛⠙⠛⠁⠀⠀⠀⠀⣿⡇⢸ ⢞⣿⣿⣷⣝⣷⣝⠦⡀⠀⠀⠀⠀⠀⠀⠀⡀⢀⠀⠀⠀⠀⠀⠛⣿⠈ ⣦⡑⠛⣟⢿⡿⣿⣷⣝⢧⡀⠀⠀⣶⣸⡇⣿⢸⣧⠀⠀⠀⠀⢸⡿⡆ ⣿⣿⣷⣮⣭⣍⡛⠻⢿⣷⠿⣶⣶⣬⣬⣁⣉⣀⣀⣁⡤⢴⣺⣾⣽⡇"#
        ));
        assert!(should_delete(
            r#"⢿⣿⣿⣿⣭⠹⠛⠛⠛⢿⣿⣿⣿⣿⡿⣿⠷⠶⠿⢻⣿⣛⣦⣙⠻⣿⣿⣿⢿⣿⠏⠀⠀⡀⠀⠈⣿⢛⣽⣜⠯⣽⠀⠀⠀⠀⠙⢿⣷⣻⡀⢿⠐⠛⢿⣾⣖⣤⡀⠀⢀⡰⠿⢷⣶⣿⡇⠻⣖⣒⣒⣶⣿⣿⡟⢙⣶⣮⣤⠀⠀⠛⠻⠗⠿⠿⣯⡆⣿⣛⣿⡿⠿⠮⡶⠼⠟⠙⠊⠁⠀⠸⢣⣿⣿⣷⡀⠀⠀⠀⠀⠠⠭⣍⡉⢩⣥⡤⠥⣤⡶⣒⠀⠀⠀⠀⠀⢰⣿⣿⣿⣿⡽⡄⠀⠀⠀⢿⣿⣆⣿⣧⢡⣾⣿⡇⣾⣿⡇⠀⠀⠀⠀⣿⡇⠃⣿⣿⣷⣻⣆⢄⠀⠈⠉⠉⠛⠛⠘⠛⠛⠛⠙⠛⠁⠀⠀⠀⠀⣿⡇⢸⢞⣿⣿⣷⣝⣷⣝⠦⡀⠀⠀⠀⠀⠀⠀⠀⡀⢀⠀⠀⠀⠀⠀⠛⣿⠈⣦⡑⠛⣟⢿⡿⣿⣷⣝⢧⡀⠀⠀⣶⣸⡇⣿⢸⣧⠀⠀⠀⠀⢸⡿⡆⣿⣿⣷⣮⣭⣍⡛⠻⢿⣷⠿⣶⣶⣬⣬⣁⣉⣀⣀⣁⡤⢴⣺⣾⣽⡇"#
        ));
        assert!(should_delete(
            r#"
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
            "#
        ));
        assert!(should_delete(
            r#"
                ———————————No stiches?———————————
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
        ));
        assert!(should_delete(
            r#"
                |￣￣￣￣￣￣￣￣￣￣￣|
                        hola
                |__________________|
                    \ (•◡•) /
                        \      /
                        ---
                        |   |
            "#
        ));
        assert!(should_delete(
            r#"_________________________________ This chat is now in cute mode AYAYA _________________________________"#
        ));
        assert!(should_delete(r#"> < > < ><> <> <> <> <> <> <> <> <>"#));
        assert!(should_delete(
            r#"................................. This chat is now in cute mode AYAYA ................................."#
        ));
        assert!(should_delete(
            r#"!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! This chat is now in cute mode AYAYA !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"#
        ));
        assert!(should_delete(
            r#"????????????????????????????????? This chat is now in cute mode AYAYA ?????????????????????????????????"#
        ));
    }
}
