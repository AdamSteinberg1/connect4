use shared::{Board, Color, ServerMessage};
use tokio::sync::mpsc::Sender;

pub enum GameState {
    Waiting {
        host_tx: Sender<ServerMessage>,
    },
    Active {
        red_tx: Sender<ServerMessage>,
        yellow_tx: Sender<ServerMessage>,
    },
    Finished,
}

pub struct GameSession {
    pub board: Board,
    pub whose_turn: Color,
    pub state: GameState,
}

impl GameSession {
    pub fn new(host_tx: Sender<ServerMessage>) -> Self {
        Self {
            board: Board::new(),
            whose_turn: Color::Red,
            state: GameState::Waiting { host_tx },
        }
    }
}