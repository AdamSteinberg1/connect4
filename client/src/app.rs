use crate::game::{Game, Input};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use shared::ColumnIndex;
use std::io;

pub struct App {
    game: Game,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            game: Game::new(),
            should_quit: false,
        }
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| frame.render_widget(&self.game, frame.area()))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        let Event::Key(key_event) = event::read()? else {
            return Ok(());
        };
        if key_event.kind != KeyEventKind::Press {
            return Ok(());
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            code => {
                if let Some(input) = key_to_input(code) {
                    self.game.handle_input(input);
                }
            }
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// translates a raw key press into a game input
fn key_to_input(code: KeyCode) -> Option<Input> {
    match code {
        KeyCode::Left => Some(Input::Left),
        KeyCode::Right => Some(Input::Right),
        KeyCode::Enter => Some(Input::Confirm),
        KeyCode::Char(c) => {
            let index = (c.to_digit(10)? as usize).checked_sub(1)?;
            ColumnIndex::new(index).ok().map(Input::SelectColumn)
        }
        _ => None,
    }
}
