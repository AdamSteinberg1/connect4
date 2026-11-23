use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, BorderType::QuadrantInside, Widget},
};

/// a bordered screen with vertically centered text and a controls line
pub struct TextScreen {
    pub text: Text<'static>,
    pub controls: Line<'static>,
}

impl Widget for TextScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(QuadrantInside)
            .border_style(Style::new().blue())
            .title_bottom(self.controls);
        let inner_area = block.inner(area);
        block.render(area, buf);

        let height = u16::try_from(self.text.height()).unwrap_or(u16::MAX);
        let [_, middle, _] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .areas(inner_area);

        self.text.centered().render(middle, buf);
    }
}
