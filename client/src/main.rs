mod app;
mod game;
mod input;
mod menu;
mod net;
mod online;
mod selector;
mod ui;

use app::App;
use std::io;

//todo highlight the winning 4
//todo animate token dropping

#[tokio::main]
async fn main() -> io::Result<()> {
    let server_addr =
        std::env::var("CONNECT4_SERVER").unwrap_or_else(|_| net::DEFAULT_SERVER.to_string());

    let mut terminal = ratatui::init();
    let result = App::new(server_addr).run(&mut terminal).await;
    ratatui::restore();
    result
}
