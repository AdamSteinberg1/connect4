use super::game_view::{GameScreen, colored, controls_line};
use super::text_screen::TextScreen;
use crate::online::{Match, Online};
use itertools::Itertools;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::{Line, Span, Text},
    widgets::Widget,
};

type Controls<'a> = &'a [(&'static str, &'static str)];

impl Widget for &Online {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            Online::EnteringCode { input, error } => {
                let code = (0..6)
                    .map(|i| input.chars().nth(i).unwrap_or('_'))
                    .join(" ");
                let mut lines = vec![
                    Line::from("Enter join code").bold(),
                    Line::raw(""),
                    Line::from(code),
                ];
                if let Some(error) = error {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(error.clone()).red());
                }
                text_screen(lines, &[("Join", "<Enter>"), ("Back", "<Esc>")]).render(area, buf);
            }
            Online::Creating | Online::Joining => {
                text_screen(
                    vec![Line::from("Waiting for the server...")],
                    &[("Cancel", "<Esc>")],
                )
                .render(area, buf);
            }
            Online::Waiting { join_code } => {
                text_screen(
                    vec![
                        Line::from("Share this join code with your opponent:"),
                        Line::raw(""),
                        Line::from(join_code.to_string()).bold(),
                        Line::raw(""),
                        Line::from("Waiting for opponent..."),
                    ],
                    &[("Cancel", "<Esc>")],
                )
                .render(area, buf);
            }
            Online::Playing(game) => {
                GameScreen {
                    heading: playing_heading(game),
                    board: &game.board,
                    selected_column: game
                        .is_my_turn()
                        .then(|| game.selector.selected())
                        .flatten(),
                    controls: controls_line(&[
                        ("Place", "<Enter>"),
                        ("Move", "<Left/Right>"),
                        ("Leave", "<Esc>"),
                        ("Quit", "<Q>"),
                    ]),
                }
                .render(area, buf);
            }
            Online::Finished { game, winner } => {
                GameScreen {
                    heading: finished_heading(game, *winner),
                    board: &game.board,
                    selected_column: None,
                    controls: controls_line(&[("Menu", "<Enter>"), ("Quit", "<Q>")]),
                }
                .render(area, buf);
            }
            Online::Ended { reason } => {
                text_screen(
                    vec![
                        Line::from(*reason).bold(),
                        Line::raw(""),
                        Line::from("Press enter to return to the menu"),
                    ],
                    &[("Menu", "<Enter>"), ("Quit", "<Q>")],
                )
                .render(area, buf);
            }
        }
    }
}

fn text_screen(lines: Vec<Line<'static>>, controls: Controls) -> TextScreen {
    TextScreen {
        text: Text::from(lines),
        controls: controls_line(controls),
    }
}

fn playing_heading(game: &Match) -> Text<'static> {
    let (title, subtitle) = if game.is_my_turn() {
        (colored("Your turn".to_string(), game.my_color), String::new())
    } else {
        (
            colored("Opponent's turn".to_string(), game.my_color.other()),
            "Waiting for opponent...".to_string(),
        )
    };
    let subtitle = match &game.notice {
        Some(error) => format!("invalid move: {error}"),
        None => subtitle,
    };
    Text::from(vec![Line::from(title).bold(), Line::from(subtitle)]).centered()
}

fn finished_heading(game: &Match, winner: Option<shared::Color>) -> Text<'static> {
    let title = match winner {
        Some(color) if color == game.my_color => colored("You won!".to_string(), color),
        Some(color) => colored("You lost".to_string(), color),
        None => Span::raw("Draw"),
    };
    Text::from(vec![
        Line::from(title).bold(),
        Line::from("Press enter to return to the menu"),
    ])
    .centered()
}
