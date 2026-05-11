use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct JoinCode([u8; LENGTH]);

#[derive(Debug, Error)]
#[error("invalid join code")]
pub struct InvalidJoinCode;

const LENGTH: usize = 6;
const CHARSET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

impl JoinCode {
    pub fn random() -> Self {
        let mut rng = rand::rng();
        let code = CHARSET
            .sample_array(&mut rng)
            .expect("There should always be enough chars in charset");
        Self(code)
    }
}

impl FromStr for JoinCode {
    type Err = InvalidJoinCode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s: [u8; LENGTH] = s.as_bytes().try_into().map_err(|_| InvalidJoinCode)?;
        if s.iter().any(|c| !CHARSET.contains(c)) {
            return Err(InvalidJoinCode);
        }
        Ok(Self(s))
    }
}

impl fmt::Display for JoinCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}
impl fmt::Debug for JoinCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}