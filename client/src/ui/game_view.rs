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
use shared::Color;

impl Widget for &Game {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(QuadrantInside)
            .border_style(Style::new().blue())
            .title_bottom(controls_line());

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

        heading(self).render(heading_layout[1], buf);
        BoardWidget {
            board: self.board(),
            selected_column: self.selected_column(),
        }
        .render(horizontal_thirds[1], buf);
    }
}

fn controls_line() -> Line<'static> {
    Line::from(vec![
        " Place ".into(),
        "<Enter> ".blue().bold(),
        "Move ".into(),
        "<Left/Right> ".blue().bold(),
        "Quit ".into(),
        "<Q> ".blue().bold(),
    ])
    .centered()
    .white()
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

fn colored(text: String, color: Color) -> Span<'static> {
    match color {
        Color::Red => text.red(),
        Color::Yellow => text.yellow(),
    }
}
