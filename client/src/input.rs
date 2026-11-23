use crate::game::Input;
use crossterm::event::KeyCode;
use shared::ColumnIndex;

/// translates a raw key press into a game input
pub fn key_to_input(code: KeyCode) -> Option<Input> {
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
