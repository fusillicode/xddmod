use fake::Dummy;
use serde::Deserialize;
use serde::Serialize;

pub mod champion;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Dummy, sqlx::Type)]
#[sqlx(transparent)]
pub struct ChampionKey(String);

impl From<String> for ChampionKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ChampionKey {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<i64> for ChampionKey {
    fn from(value: i64) -> Self {
        Self(value.to_string())
    }
}
