use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JoinCode(String);

#[derive(Debug, Error)]
#[error("invalid join code")]
pub struct InvalidJoinCode;

impl FromStr for JoinCode {
    type Err = InvalidJoinCode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const LENGTH: usize = 6;
        const CHARSET: &'static str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

        if s.len() != LENGTH {
            return Err(InvalidJoinCode);
        }
        if s.chars().any(|c| !CHARSET.contains(c)) {
            return Err(InvalidJoinCode);
        }
        Ok(Self(s.to_owned()))
    }
}

impl fmt::Display for JoinCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
