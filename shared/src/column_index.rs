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

    pub fn increment(mut self) {
        if self.0 < Self::MAX {
            self.0 += 1;
        }
    }

    pub fn decrement(mut self) {
        if self.0 > 0 {
            self.0 -= 1;
        }
    }

    pub fn right_most() -> Self {
        ColumnIndex(Self::MAX)
    }

    pub fn left_most() -> Self {
        ColumnIndex(0)
    }
}
