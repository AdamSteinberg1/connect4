use crate::board::{Board, Color};
use crate::column_index::ColumnIndex;
use crate::join_code::JoinCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum MoveError {
    #[error("not your turn")]
    NotYourTurn,
    #[error("column is full")]
    ColumnFull,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    CreateGame,
    JoinGame { code: JoinCode },
    PlayMove { column: ColumnIndex },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    GameCreated {
        join_code: JoinCode,
    },
    GameStarted {
        your_color: Color,
    },
    MovePlayed {
        column: ColumnIndex,
        color: Color,
        board: Board,
    },
    InvalidMove(MoveError),
    GameOver {
        winner: Option<Color>,
    },
    OpponentDisconnected,
    GameNotFound,
    GameFull,
    CannotJoinOwnGame
}
