use super::game_view::controls_line;
use super::text_screen::TextScreen;
use crate::menu::{Menu, MenuItem};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::{Line, Text},
    widgets::Widget,
};

impl Widget for &Menu {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![Line::from("Connect 4").bold(), Line::raw("")];
        lines.extend(MenuItem::ALL.iter().enumerate().map(|(index, item)| {
            if index == self.selected_index() {
                Line::from(format!("> {} <", item.label())).bold()
            } else {
                Line::from(item.label())
            }
        }));
        if let Some(error) = self.error() {
            lines.push(Line::raw(""));
            lines.push(Line::from(error.to_owned()).red());
        }

        TextScreen {
            text: Text::from(lines),
            controls: controls_line(&[
                ("Select", "<Up/Down>"),
                ("Choose", "<Enter>"),
                ("Quit", "<Q>"),
            ]),
        }
        .render(area, buf);
    }
}
