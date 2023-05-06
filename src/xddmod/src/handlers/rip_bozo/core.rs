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
            if message
                .badges
                .iter()
                .any(|b| b.name == "moderator" || b.name == "broadcaster")
            {
                return;
            }

            if should_delete(&message.message_text) {
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
    static ref ANY_EMOJI_REGEX: Regex = Regex::new(r#"\p{Emoji}"#).unwrap();
}

fn should_delete(message_text: &str) -> bool {
    if ANY_EMOJI_REGEX.is_match(message_text) {
        return false;
    }

    let no_whitespaces = message_text.chars().filter(|c| !c.is_whitespace());
    let (ascii, not_ascii): (Vec<char>, Vec<char>) = no_whitespaces.clone().partition(char::is_ascii);

    let not_ascii_count = not_ascii.len();
    if not_ascii_count == 0 {
        return false;
    }

    if not_ascii.iter().all(|x| x == &'…') {
        return false;
    }

    let ascii_count = ascii.len();
    let not_ascii_perc = (not_ascii_count as f64 / (not_ascii_count + ascii_count) as f64) * 100.0;

    not_ascii_perc >= 45.0
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
        assert!(should_delete(r#"…ö"#));
        assert!(!should_delete(
            r#""El presidente del Congreso, que aún no ha manifestado si se adherirá o no a la iniciativa del ministro de Industria, no quiso dar trascendencia al asunto, «que no tiene más valor que el de una anécdota y el de una corbata regalada»."#
        ));
        assert!(!should_delete(
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
        assert!(!should_delete(r#"🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲"#));
        assert!(!should_delete(
            r#"🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲"#
        ));

        assert!(!should_delete(
            r#"🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲🥲"#
        ));
        assert!(!should_delete(
            r#"🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲 🥲"#
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
                ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠿⠋⠉⠉⠉⠄⠈⠉⠙⠿⣿⣿⣿⣿⣿⣿⣿
                ⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟⠋⠁⠄⠄⠄⣀⣤⣴⣶⣤⣤⣀⠈⢿⣿⣿⣿⣿⣿
                ⣿⣿⣿⣿⣿⣿⡿⠋⠁⠄⠄⠄⠄⢸⣿⣿⣿⣿⣿⣿⣿⣿⣷⡄⢻⣿⣿⣿⣿
                ⣿⣿⣿⣿⣿⡟⠁⠄⠄⠄⠄⠄⠉⣛⣛⣛⡛⢻⣿⣿⣿⣿⣿⣿⡀⢻⣿⣿⣿
                ⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠄⣾⣿⡟⢱⣆⠄⠿⢿⣿⣿⣯⠬⣙⡇⠘⣿⣿⣿
                ⣿⣿⣿⡇⠄⠄⠄⠄⠄⠄⠄⣻⣿⣿⣭⣵⣦⠄⣠⣿⡈⢿⣀⣸⡇⠄⣿⣿⣿
                ⣿⣿⣿⣇⠄⠄⠄⠄⠄⠄⢰⣿⣿⣿⣿⣿⡿⠃⢨⠟⣷⣿⣿⣿⠃⠄⢿⣿⣿
                ⣿⣿⣿⣿⠄⠄⠄⠄⠄⠄⠸⣿⣿⣿⠟⠁⠄⠄⠄⠄⠹⣿⣿⣿⠄⠄⠄⣿⣿
                ⣿⣿⣿⣿⡄⠄⠄⠄⠄⠄⠄⣠⣿⣿⡿⢂⣀⢸⣦⠄⠄⣹⣿⠇⠄⠄⣼⣿⣿
                ⣿⣿⣿⣿⣿⣧⠄⠄⢀⣴⣿⣿⣟⣉⣴⣿⠇⣠⣾⠂⠄⠈⠄⠄⢀⣼⣿⣿⣿
                ⣿⣿⣿⣿⡿⠟⢀⣴⣿⣿⣿⣿⣿⣿⣿⣵⣿⡿⣣⠄⠄⠄⠄⣰⣿⣿⠿⠋⠉
                ⠛⠋⠉⠁⠄⣠⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣵⡿⠃⠄⠄⠄⢠⣿⠟⠁⠄⠄⠄
                ⠄⠄⠄⠄⣼⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠈⠁⠄⠄⠄⠄⠄
                ⠄⠄⢀⣰⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄
            "#
        ));
        assert!(should_delete(
            r#"⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠿⠋⠉⠉⠉⠄⠈⠉⠙⠿⣿⣿⣿⣿⣿⣿⣿ ⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟⠋⠁⠄⠄⠄⣀⣤⣴⣶⣤⣤⣀⠈⢿⣿⣿⣿⣿⣿ ⣿⣿⣿⣿⣿⣿⡿⠋⠁⠄⠄⠄⠄⢸⣿⣿⣿⣿⣿⣿⣿⣿⣷⡄⢻⣿⣿⣿⣿ ⣿⣿⣿⣿⣿⡟⠁⠄⠄⠄⠄⠄⠉⣛⣛⣛⡛⢻⣿⣿⣿⣿⣿⣿⡀⢻⣿⣿⣿ ⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠄⣾⣿⡟⢱⣆⠄⠿⢿⣿⣿⣯⠬⣙⡇⠘⣿⣿⣿ ⣿⣿⣿⡇⠄⠄⠄⠄⠄⠄⠄⣻⣿⣿⣭⣵⣦⠄⣠⣿⡈⢿⣀⣸⡇⠄⣿⣿⣿ ⣿⣿⣿⣇⠄⠄⠄⠄⠄⠄⢰⣿⣿⣿⣿⣿⡿⠃⢨⠟⣷⣿⣿⣿⠃⠄⢿⣿⣿ ⣿⣿⣿⣿⠄⠄⠄⠄⠄⠄⠸⣿⣿⣿⠟⠁⠄⠄⠄⠄⠹⣿⣿⣿⠄⠄⠄⣿⣿ ⣿⣿⣿⣿⡄⠄⠄⠄⠄⠄⠄⣠⣿⣿⡿⢂⣀⢸⣦⠄⠄⣹⣿⠇⠄⠄⣼⣿⣿ ⣿⣿⣿⣿⣿⣧⠄⠄⢀⣴⣿⣿⣟⣉⣴⣿⠇⣠⣾⠂⠄⠈⠄⠄⢀⣼⣿⣿⣿ ⣿⣿⣿⣿⡿⠟⢀⣴⣿⣿⣿⣿⣿⣿⣿⣵⣿⡿⣣⠄⠄⠄⠄⣰⣿⣿⠿⠋⠉ ⠛⠋⠉⠁⠄⣠⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣵⡿⠃⠄⠄⠄⢠⣿⠟⠁⠄⠄⠄ ⠄⠄⠄⠄⣼⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠈⠁⠄⠄⠄⠄⠄ ⠄⠄⢀⣰⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄ "#
        ));
        assert!(should_delete(
            r#"⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠿⠋⠉⠉⠉⠄⠈⠉⠙⠿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟⠋⠁⠄⠄⠄⣀⣤⣴⣶⣤⣤⣀⠈⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠁⠄⠄⠄⠄⢸⣿⣿⣿⣿⣿⣿⣿⣿⣷⡄⢻⣿⣿⣿⣿⣿⣿⣿⣿⣿⡟⠁⠄⠄⠄⠄⠄⠉⣛⣛⣛⡛⢻⣿⣿⣿⣿⣿⣿⡀⢻⣿⣿⣿⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠄⣾⣿⡟⢱⣆⠄⠿⢿⣿⣿⣯⠬⣙⡇⠘⣿⣿⣿⣿⣿⣿⡇⠄⠄⠄⠄⠄⠄⠄⣻⣿⣿⣭⣵⣦⠄⣠⣿⡈⢿⣀⣸⡇⠄⣿⣿⣿⣿⣿⣿⣇⠄⠄⠄⠄⠄⠄⢰⣿⣿⣿⣿⣿⡿⠃⢨⠟⣷⣿⣿⣿⠃⠄⢿⣿⣿⣿⣿⣿⣿⠄⠄⠄⠄⠄⠄⠸⣿⣿⣿⠟⠁⠄⠄⠄⠄⠹⣿⣿⣿⠄⠄⠄⣿⣿⣿⣿⣿⣿⡄⠄⠄⠄⠄⠄⠄⣠⣿⣿⡿⢂⣀⢸⣦⠄⠄⣹⣿⠇⠄⠄⣼⣿⣿⣿⣿⣿⣿⣿⣧⠄⠄⢀⣴⣿⣿⣟⣉⣴⣿⠇⣠⣾⠂⠄⠈⠄⠄⢀⣼⣿⣿⣿⣿⣿⣿⣿⡿⠟⢀⣴⣿⣿⣿⣿⣿⣿⣿⣵⣿⡿⣣⠄⠄⠄⠄⣰⣿⣿⠿⠋⠉⠛⠋⠉⠁⠄⣠⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣵⡿⠃⠄⠄⠄⢠⣿⠟⠁⠄⠄⠄⠄⠄⠄⠄⣼⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠈⠁⠄⠄⠄⠄⠄⠄⠄⢀⣰⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄"#
        ));
        assert!(should_delete(
            r#"░░░░█─────────────█──▀── ░░░░▓█───────▄▄▀▀█────── ░░░░▒░█────▄█▒░░▄░█───── ░░░░░░░▀▄─▄▀▒▀▀▀▄▄▀──DO─ ░░░░░░░░░█▒░░░░▄▀───YOU─ ▒▒▒░░░░▄▀▒░░░░▄▀───LIKE─ ▓▓▓▓▒░█▒░░░░░█▄───WHAT─ █████▀▒░░░░░█░▀▄───YOU── █████▒▒░░░▒█░░░▀▄─SEE?── ███▓▓▒▒▒▀▀▀█▄░░░░█────── ▓██▓▒▒▒▒▒▒▒▒▒█░░░░█───── ▓▓█▓▒▒▒▒▒▒▓▒▒█░░░░░█──── ░▒▒▀▀▄▄▄▄█▄▄▀░░░░░░░█─ "#
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
    }
}
