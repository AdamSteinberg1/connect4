use crate::selector::ColumnSelector;
use shared::{Board, Color, ColumnIndex};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    InProgress { turn: Color },
    Won(Color),
    Draw,
}

/// player intent, independent of which key produced it
#[derive(Clone, Copy, Debug)]
pub enum Input {
    Left,
    Right,
    SelectColumn(ColumnIndex),
    Confirm,
}

pub struct Game {
    board: Board,
    status: Status,
    selector: ColumnSelector,
    last_move_invalid: bool,
}

impl Game {
    pub fn new() -> Self {
        Self {
            board: Board::new(),
            status: Status::InProgress { turn: Color::Red },
            selector: ColumnSelector::default(),
            last_move_invalid: false,
        }
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn selected_column(&self) -> Option<ColumnIndex> {
        self.selector.selected()
    }

    pub fn last_move_invalid(&self) -> bool {
        self.last_move_invalid
    }

    pub fn handle_input(&mut self, input: Input) {
        match (self.status, input) {
            (Status::InProgress { .. }, Input::Left) => self.selector.move_left(),
            (Status::InProgress { .. }, Input::Right) => self.selector.move_right(),
            (Status::InProgress { .. }, Input::SelectColumn(col)) => self.selector.select(col),
            (Status::InProgress { turn }, Input::Confirm) => {
                if let Some(col) = self.selector.selected() {
                    self.take_turn(col, turn);
                }
            }
            (Status::Won(_) | Status::Draw, Input::Confirm) => *self = Self::new(),
            _ => {}
        }
    }

    fn take_turn(&mut self, column: ColumnIndex, color: Color) {
        if self.board.play_turn(column, color).is_err() {
            self.last_move_invalid = true;
            return;
        }
        self.last_move_invalid = false;

        self.status = if let Some(winner) = self.board.get_winner() {
            Status::Won(winner)
        } else if self.board.is_full() {
            Status::Draw
        } else {
            Status::InProgress {
                turn: color.other(),
            }
        };
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
