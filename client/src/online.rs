use crate::game::Input;
use crate::input::key_to_input;
use crate::selector::ColumnSelector;
use crossterm::event::KeyCode;
use shared::{Board, ClientMessage, Color, JoinCode, MoveError, ServerMessage};

// must match the length of a JoinCode
const JOIN_CODE_LENGTH: usize = 6;

/// the client's view of an in-progress online game
pub struct Match {
    pub my_color: Color,
    pub board: Board,
    pub selector: ColumnSelector,
    pub notice: Option<MoveError>,
}

impl Match {
    fn new(my_color: Color) -> Self {
        Self {
            my_color,
            board: Board::new(),
            selector: ColumnSelector::default(),
            notice: None,
        }
    }

    pub fn is_my_turn(&self) -> bool {
        self.board.whose_turn() == self.my_color
    }

    /// input is ignored while waiting for the opponent.
    /// the board only changes when the server confirms a move
    fn handle_input(&mut self, input: Input) -> Option<ClientMessage> {
        if !self.is_my_turn() {
            return None;
        }
        match input {
            Input::Left => self.selector.move_left(),
            Input::Right => self.selector.move_right(),
            Input::SelectColumn(column) => self.selector.select(column),
            Input::Confirm => {
                return self
                    .selector
                    .selected()
                    .map(|column| ClientMessage::PlayMove { column });
            }
        }
        None
    }
}

pub enum Online {
    EnteringCode {
        input: String,
        error: Option<String>,
    },
    /// CreateGame sent, waiting for the join code
    Creating,
    /// JoinGame sent, waiting for the game to start
    Joining,
    Waiting {
        join_code: JoinCode,
    },
    Playing(Match),
    Finished {
        game: Match,
        winner: Option<Color>,
    },
    Ended {
        reason: &'static str,
    },
}

impl Online {
    /// true while the server holds a session for this connection that
    /// would need to be abandoned
    pub fn is_in_session(&self) -> bool {
        matches!(
            self,
            Online::Creating | Online::Joining | Online::Waiting { .. } | Online::Playing(_)
        )
    }

    pub fn is_over(&self) -> bool {
        matches!(self, Online::Finished { .. } | Online::Ended { .. })
    }

    pub fn handle_key(self, code: KeyCode) -> (Self, Option<ClientMessage>) {
        match self {
            Online::EnteringCode { mut input, error } => match code {
                KeyCode::Char(c) if c.is_ascii_alphanumeric() && input.len() < JOIN_CODE_LENGTH => {
                    input.push(c.to_ascii_uppercase());
                    (Online::EnteringCode { input, error: None }, None)
                }
                KeyCode::Backspace => {
                    input.pop();
                    (Online::EnteringCode { input, error: None }, None)
                }
                KeyCode::Enter => match input.parse::<JoinCode>() {
                    Ok(join_code) => (Online::Joining, Some(ClientMessage::JoinGame { join_code })),
                    Err(_) => (
                        Online::EnteringCode {
                            input,
                            error: Some("That is not a valid join code".to_string()),
                        },
                        None,
                    ),
                },
                _ => (Online::EnteringCode { input, error }, None),
            },
            Online::Playing(mut game) => {
                let message = key_to_input(code).and_then(|input| game.handle_input(input));
                (Online::Playing(game), message)
            }
            other => (other, None),
        }
    }

    pub fn on_message(self, message: ServerMessage) -> Self {
        match (self, message) {
            (Online::Creating, ServerMessage::GameCreated { join_code }) => {
                Online::Waiting { join_code }
            }
            (Online::Waiting { .. } | Online::Joining, ServerMessage::GameStarted { your_color }) => {
                Online::Playing(Match::new(your_color))
            }
            (Online::Joining, ServerMessage::JoinFailed) => Online::EnteringCode {
                input: String::new(),
                error: Some("No waiting game has that code".to_string()),
            },
            (Online::Playing(mut game), ServerMessage::MovePlayed { board, .. }) => {
                game.board = board;
                game.notice = None;
                Online::Playing(game)
            }
            (Online::Playing(mut game), ServerMessage::InvalidMove(error)) => {
                game.notice = Some(error);
                Online::Playing(game)
            }
            (Online::Playing(game), ServerMessage::GameOver { winner }) => {
                Online::Finished { game, winner }
            }
            (Online::Playing(_), ServerMessage::OpponentDisconnected) => Online::Ended {
                reason: "Your opponent disconnected",
            },
            (state, _) => state,
        }
    }

    pub fn on_connection_lost(self) -> Self {
        match self {
            Online::Finished { .. } | Online::Ended { .. } => self,
            _ => Online::Ended {
                reason: "Lost connection to the server",
            },
        }
    }
}
