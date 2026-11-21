mod app;
mod game;
mod ui;

use app::App;
use std::io;

//todo highlight the winning 4
//todo make start menu
//todo animate token dropping

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);
    ratatui::restore();
    result
}
