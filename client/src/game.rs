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
    selected_column: Option<ColumnIndex>,
    last_move_invalid: bool,
}

impl Game {
    pub fn new() -> Self {
        Self {
            board: Board::new(),
            status: Status::InProgress { turn: Color::Red },
            selected_column: None,
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
        self.selected_column
    }

    pub fn last_move_invalid(&self) -> bool {
        self.last_move_invalid
    }

    pub fn handle_input(&mut self, input: Input) {
        match (self.status, input) {
            (Status::InProgress { .. }, Input::Left) => self.decrement_selection(),
            (Status::InProgress { .. }, Input::Right) => self.increment_selection(),
            (Status::InProgress { .. }, Input::SelectColumn(col)) => {
                self.selected_column = Some(col);
            }
            (Status::InProgress { turn }, Input::Confirm) => {
                if let Some(col) = self.selected_column {
                    self.take_turn(col, turn);
                }
            }
            (Status::Won(_) | Status::Draw, Input::Confirm) => *self = Self::new(),
            _ => {}
        }
    }

    fn increment_selection(&mut self) {
        self.selected_column = Some(match self.selected_column {
            Some(col) => col.increment(),
            None => ColumnIndex::right_most(),
        });
    }

    fn decrement_selection(&mut self) {
        self.selected_column = Some(match self.selected_column {
            Some(col) => col.decrement(),
            None => ColumnIndex::left_most(),
        });
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
