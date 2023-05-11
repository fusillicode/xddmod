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

const NOT_ASCII_WHITELIST: [&str; 3] = ["\u{e0000}", "…", "？"];

fn should_delete(message_text: &str) -> bool {
    let graphemes: Vec<&str> = UnicodeSegmentation::graphemes(message_text, true).collect();

    let (_whitespaces_count, ascii, emojis, not_ascii): (usize, Vec<&str>, Vec<&str>, Vec<&str>) =
        graphemes.into_iter().fold(
            (0, vec![], vec![], vec![]),
            |(mut whitespaces_count, mut ascii, mut emojis, mut not_ascii), g| {
                match g.is_ascii() {
                    true | false if g.trim().is_empty() => whitespaces_count += 1,
                    true => ascii.push(g),
                    false if EMOJI_REGEX.is_match(g) => emojis.push(g),
                    false if NOT_ASCII_WHITELIST.contains(&g) => (),
                    false => not_ascii.push(g),
                }

                (whitespaces_count, ascii, emojis, not_ascii)
            },
        );

    let ascii_count = ascii.len();
    let emojis_count = emojis.len();
    let not_ascii_count = not_ascii.len();
    let no_whitespaces_count = emojis_count + not_ascii_count + ascii_count;

    if emojis_count == no_whitespaces_count {
        return no_whitespaces_count > 24;
    }

    if not_ascii_count == 0 {
        return false;
    }

    let not_ascii_perc = (not_ascii_count as f64 / (not_ascii_count + ascii_count) as f64) * 100.0;
    not_ascii_perc > 45.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_delete() {
        assert!(!should_delete(r#""#));
        assert!(!should_delete(r#" "#));
        assert!(!should_delete(r#"ciao"#));
        assert!(!should_delete(r#"..."#));
        assert!(!should_delete(r#"......"#));
        assert!(!should_delete(r#"........."#));
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
        assert!(should_delete(r#"…ö"#));
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
                        insert
                        text
                        here
                |＿＿＿＿＿＿＿＿＿＿＿|
                    \ (•◡•) /
                        \      /
                        ---
                        |   |
            "#
        ));
        assert!(should_delete(
            r#"
                |￣￣￣￣￣￣￣￣￣￣￣|
                        hola
                |＿＿＿＿＿＿＿＿＿＿＿|
                    \ (•◡•) /
                        \      /
                        ---
                        |   |
            "#
        ));
        // assert!(should_delete(
        //     r#"_________________________________ This chat is now in cute mode AYAYA
        // _________________________________"# ));
    }
}
