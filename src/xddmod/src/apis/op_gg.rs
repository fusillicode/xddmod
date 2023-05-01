use std::fmt;

use fake::Dummy;
use serde::Deserialize;
use serde::Serialize;

pub mod games;
pub mod summoners;

#[derive(Clone, Copy, Debug, Dummy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    Br,
    Eune,
    Euw,
    Jp,
    Kr,
    Lan,
    Las,
    Na,
    Oce,
    Ph,
    Ru,
    Sg,
    Th,
    Tr,
    Tw,
    Vn,
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Region::Br => "br",
            Region::Eune => "eune",
            Region::Euw => "euw",
            Region::Jp => "jp",
            Region::Kr => "kr",
            Region::Lan => "lan",
            Region::Las => "las",
            Region::Na => "na",
            Region::Oce => "oce",
            Region::Ph => "ph",
            Region::Ru => "ru",
            Region::Sg => "sg",
            Region::Th => "th",
            Region::Tr => "tr",
            Region::Tw => "tw",
            Region::Vn => "vn",
        };
        write!(f, "{}", s)
    }
}

pub const OP_GG_API: &str = "https://op.gg/api/v1.0/internal/bypass";
