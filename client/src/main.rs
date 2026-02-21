mod board;

use crate::board::{Board, Color};
use crate::GameState::{Complete, Exit, Playing};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use itertools::Itertools;
use ratatui::layout::Direction::{Horizontal, Vertical};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Span, Text};
use ratatui::widgets::BorderType::QuadrantInside;
use ratatui::widgets::Paragraph;
use ratatui::{
    buffer::Buffer, layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Padding, Widget},
    DefaultTerminal,
    Frame,
};
use std::io;
//todo highlight the winning 4
//todo make start menu
//todo animate token dropping

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::new().run(&mut terminal);
    ratatui::restore();
    app_result
}

pub struct App {
    board_widget: BoardWidget,
    current_player: Color,
    title: String,
    subtitle: String,
    game_state: GameState,
}

#[derive(PartialEq)]
enum GameState {
    Playing,
    Complete,
    Exit,
}

impl App {
    pub fn new() -> Self {
        Self {
            board_widget: Default::default(),
            current_player: Color::Red,
            title: "Red's turn".to_owned(),
            subtitle: String::new(),
            game_state: Playing,
        }
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while self.game_state != Exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn increment_selection(&mut self) {
        if let Some(col_idx) = self.board_widget.selected_column.as_mut() {
            *col_idx = 6.min(*col_idx + 1)
        } else {
            //default to right-most column
            self.board_widget.selected_column = Some(6);
        }
    }

    fn decrement_selection(&mut self) {
        if let Some(col_idx) = self.board_widget.selected_column.as_mut() {
            *col_idx = col_idx.saturating_sub(1);
        } else {
            //default to left-most column
            self.board_widget.selected_column = Some(0);
        }
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> io::Result<()> {
        let Event::Key(key_event) = event::read()? else {
            return Ok(());
        };
        if key_event.kind != KeyEventKind::Press {
            return Ok(());
        }

        self.handle_key_event(&key_event);

        Ok(())
    }

    fn handle_key_event(&mut self, key_event: &KeyEvent) {
        match (&self.game_state, key_event.code) {
            (_, KeyCode::Char('q') | KeyCode::Esc) => self.exit(),
            (Playing, KeyCode::Char(digit)) => {
                if let Some(num) = digit.to_digit(10)
                    && num <= 7
                {
                    let col_idx = (num - 1) as usize;
                    self.board_widget.selected_column = Some(col_idx);
                };
            }
            (Playing, KeyCode::Left) => self.decrement_selection(),
            (Playing, KeyCode::Right) => self.increment_selection(),
            (Playing, KeyCode::Enter) => {
                if let Some(col_idx) = self.board_widget.selected_column {
                    self.take_turn(col_idx);
                }
            }
            (Complete, KeyCode::Enter) => {
                *self = Self::new();
            }
            _ => {}
        }
    }

    fn take_turn(&mut self, column_idx: usize) {
        let turn_result = self
            .board_widget
            .board
            .play_turn(column_idx, self.current_player);
        if turn_result.is_err() {
            self.subtitle = "invalid move, try again".to_owned();
            return;
        }

        if let Some(winner) = self.board_widget.board.get_winner() {
            self.title = match winner {
                Color::Red => "Red won!".to_owned(),
                Color::Yellow => "Yellow won!".to_owned(),
            };
            self.subtitle = "Press enter to play again".to_owned();
            self.game_state = Complete;
            return;
        }

        if self.board_widget.board.is_full() {
            self.title = "Draw".to_owned();
            self.subtitle = "Press enter to play again".to_owned();
            self.game_state = Complete;
            return;
        }

        self.subtitle.clear();

        self.current_player = match self.current_player {
            Color::Yellow => Color::Red,
            Color::Red => Color::Yellow,
        };
        self.title = match self.current_player {
            Color::Red => "Red's turn".to_string(),
            Color::Yellow => "Yellow's turn".to_string(),
        }
    }

    fn exit(&mut self) {
        self.game_state = Exit;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create the border block and render it first
        let controls = Line::from(vec![
            " Place ".into(),
            "<Enter> ".blue().bold(),
            "Move ".into(),
            "<Left/Right> ".blue().bold(),
            "Quit ".into(),
            "<Q> ".blue().bold(),
        ])
        .centered()
        .white();
        let block = Block::bordered()
            .border_type(QuadrantInside)
            .border_style(Style::new().blue())
            .title_bottom(controls);

        // Get the inner area (inside the border) and render the block
        let inner_area = block.inner(area);
        block.render(area, buf);

        // Now lay out content within the inner area
        let vertical_thirds = Layout::default()
            .direction(Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(9),
                Constraint::Min(0),
            ])
            .split(inner_area);
        let horizontal_thirds = Layout::default()
            .direction(Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(17),
                Constraint::Min(0),
            ])
            .split(vertical_thirds[1]);

        let heading_layout = Layout::default()
            .direction(Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Min(2)])
            .split(vertical_thirds[0]);
        let title_text = match self.title.clone() {
            text if text.contains("Red") => text.red(),
            text if text.contains("Yellow") => text.yellow(),
            text => text.into(),
        };
        let heading = Text::from(vec![
            Line::from(title_text).bold(),
            self.subtitle.clone().into(),
        ])
        .centered();
        heading.render(heading_layout[1], buf);
        self.board_widget.render(horizontal_thirds[1], buf);
    }
}

#[derive(Default)]
struct BoardWidget {
    board: Board,
    selected_column: Option<usize>,
}

impl Widget for &BoardWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split area to reserve space for numbers below
        let layout = Layout::default()
            .direction(Vertical)
            .constraints([
                Constraint::Min(0),    // Table area
                Constraint::Length(1), // Numbers row
            ])
            .split(area);

        let text = self
            .board
            .rows()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .map(|(col_idx, slot)| {
                        let span = match slot {
                            None => "◯".into(),
                            Some(Color::Red) => "◉".red(),
                            Some(Color::Yellow) => "◉".yellow(),
                        };
                        if self.selected_column == Some(col_idx) {
                            span.on_dark_gray()
                        } else {
                            span
                        }
                    })
                    .intersperse(" ".into())
                    .collect::<Line>()
            })
            .collect::<Text>();
        let paragraph = Paragraph::new(text)
            .centered()
            .block(Block::bordered().padding(Padding::horizontal(1)));
        paragraph.render(layout[0], buf);

        // Render numbers below the block
        let numbers = (0..7)
            .map(|i| {
                let span = Span::from((i + 1).to_string());
                if self.selected_column == Some(i) {
                    span.on_dark_gray()
                } else {
                    span
                }
            })
            .intersperse(Span::from(" "))
            .collect::<Line>()
            .centered();
        numbers.render(layout[1], buf);
    }
}
