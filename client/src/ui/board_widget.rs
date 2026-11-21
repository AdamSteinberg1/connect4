use itertools::Itertools;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction::Vertical, Layout, Rect},
    style::Stylize,
    text::{Line, Span, Text},
    widgets::{Block, Padding, Paragraph, Widget},
};
use shared::{Board, Color, ColumnIndex};

pub struct BoardWidget<'a> {
    pub board: &'a Board,
    pub selected_column: Option<ColumnIndex>,
}

impl BoardWidget<'_> {
    fn is_selected(&self, col_idx: usize) -> bool {
        self.selected_column.map(ColumnIndex::as_usize) == Some(col_idx)
    }
}

impl Widget for BoardWidget<'_> {
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
                        if self.is_selected(col_idx) {
                            span.on_dark_gray()
                        } else {
                            span
                        }
                    })
                    .intersperse(" ".into())
                    .collect::<Line>()
            })
            .collect::<Text>();
        Paragraph::new(text)
            .centered()
            .block(Block::bordered().padding(Padding::horizontal(1)))
            .render(layout[0], buf);

        // Render numbers below the block
        (0..7)
            .map(|i| {
                let span = Span::from((i + 1).to_string());
                if self.is_selected(i) {
                    span.on_dark_gray()
                } else {
                    span
                }
            })
            .intersperse(Span::from(" "))
            .collect::<Line>()
            .centered()
            .render(layout[1], buf);
    }
}
