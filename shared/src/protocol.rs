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
    #[error("you have to join a game before you can play a move")]
    NotInGame,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Request to create a new game and receive a join code
    CreateGame,
    /// Request to join an existing game by join code
    JoinGame { join_code: JoinCode },
    /// Play a move in the given column
    PlayMove { column: ColumnIndex },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Sent to the host after CreateGame succeeds; contains the code to share with an opponent
    GameCreated { join_code: JoinCode },
    /// Sent to both players when the second player joins; tells each player their color
    GameStarted { your_color: Color },
    /// Sent to both players after a valid move; contains the move and the resulting board state
    MovePlayed {
        column: ColumnIndex,
        color: Color,
        board: Board,
    },
    /// Sent to the player who made an invalid move
    InvalidMove(MoveError),
    /// Sent to both players when the game ends; winner is None on a draw
    GameOver { winner: Option<Color> },
    /// Sent to the remaining player when their opponent disconnects
    OpponentDisconnected,
    /// Sent when a JoinGame request uses a code that doesn't match any waiting game
    JoinFailed,
    /// Sent when the host sends any message other than CreateGame before an opponent has joined
    GameNotStarted,
    /// Sent when a connected player sends CreateGame or JoinGame after the game is already underway
    GameAlreadyStarted,
}
