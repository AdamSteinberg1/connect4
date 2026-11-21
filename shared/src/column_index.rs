use serde::{Deserialize, Serialize};
use thiserror::Error;

const ROW_SIZE: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnIndex(usize);

#[derive(Debug, Error)]
#[error("column index must be between 0 and 6")]
pub struct InvalidColumnIndex;

impl ColumnIndex {
    const MAX: usize = ROW_SIZE - 1;

    pub fn new(idx: usize) -> Result<Self, InvalidColumnIndex> {
        if idx > Self::MAX {
            return Err(InvalidColumnIndex);
        }
        Ok(Self(idx))
    }

    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn increment(self) -> Self {
        if self.0 < Self::MAX {
            Self(self.0 + 1)
        } else {
            Self::right_most()
        }
    }

    pub fn decrement(self) -> Self {
        if self.0 > 0 {
            Self(self.0 - 1)
        } else {
            Self::left_most()
        }
    }

    pub fn right_most() -> Self {
        Self(Self::MAX)
    }

    pub fn left_most() -> Self {
        Self(0)
    }
}
