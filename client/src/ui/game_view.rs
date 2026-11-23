use super::board_widget::BoardWidget;
use crate::game::{Game, Status};
use ratatui::{
    buffer::Buffer,
    layout::{
        Constraint,
        Direction::{Horizontal, Vertical},
        Layout, Rect,
    },
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType::QuadrantInside, Widget},
};
use shared::{Board, Color, ColumnIndex};

/// the board screen shared by local and online games
pub struct GameScreen<'a> {
    pub heading: Text<'static>,
    pub board: &'a Board,
    pub selected_column: Option<ColumnIndex>,
    pub controls: Line<'static>,
}

impl Widget for GameScreen<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(QuadrantInside)
            .border_style(Style::new().blue())
            .title_bottom(self.controls);

        let inner_area = block.inner(area);
        block.render(area, buf);

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

        self.heading.render(heading_layout[1], buf);
        BoardWidget {
            board: self.board,
            selected_column: self.selected_column,
        }
        .render(horizontal_thirds[1], buf);
    }
}

impl Widget for &Game {
    fn render(self, area: Rect, buf: &mut Buffer) {
        GameScreen {
            heading: heading(self),
            board: self.board(),
            selected_column: self.selected_column(),
            controls: controls_line(&[
                ("Place", "<Enter>"),
                ("Move", "<Left/Right>"),
                ("Menu", "<Esc>"),
                ("Quit", "<Q>"),
            ]),
        }
        .render(area, buf);
    }
}

/// builds the bottom border hint line from (action, key) pairs
pub fn controls_line(controls: &[(&'static str, &'static str)]) -> Line<'static> {
    let spans: Vec<Span<'static>> = std::iter::once(Span::raw(" "))
        .chain(controls.iter().flat_map(|(action, key)| {
            [
                Span::raw(format!("{action} ")),
                Span::styled(format!("{key} "), Style::new().blue().bold()),
            ]
        }))
        .collect();
    Line::from(spans).centered().white()
}

fn heading(game: &Game) -> Text<'static> {
    let title = match game.status() {
        Status::InProgress { turn } => colored(format!("{}'s turn", color_name(turn)), turn),
        Status::Won(winner) => colored(format!("{} won!", color_name(winner)), winner),
        Status::Draw => Span::raw("Draw"),
    };
    let subtitle = match game.status() {
        Status::InProgress { .. } if game.last_move_invalid() => "invalid move, try again",
        Status::InProgress { .. } => "",
        Status::Won(_) | Status::Draw => "Press enter to play again",
    };
    Text::from(vec![Line::from(title).bold(), subtitle.into()]).centered()
}

fn color_name(color: Color) -> &'static str {
    match color {
        Color::Red => "Red",
        Color::Yellow => "Yellow",
    }
}

pub fn colored(text: String, color: Color) -> Span<'static> {
    match color {
        Color::Red => text.red(),
        Color::Yellow => text.yellow(),
    }
}
