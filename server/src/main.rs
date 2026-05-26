mod matchmaker;
mod session;
mod connection;

use crate::connection::Connection;
use crate::matchmaker::{Matchmaker, MatchmakingMessage};
use crate::session::SessionMessage;
use anyhow::Result;
use shared::Color;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::try_join;

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = mpsc::channel::<MatchmakingMessage>(32);

    let matchmaker = Matchmaker::new(rx);
    let connection_handler = handle_connections(tx);
    try_join!(matchmaker.run(), connection_handler)?;
    Ok(())
}

async fn handle_connections(tx: mpsc::Sender<MatchmakingMessage>) -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Listening on port 8080...");

    loop {
        let (stream, _) = listener.accept().await?;
        let connection = Connection::new(stream, tx.clone());
        tokio::spawn(connection.process());
    }
}

//info the matchmaker sends to the connection, so that the connection can connect to the session
#[derive(Debug)]
struct SessionInfo {
    session_tx: mpsc::Sender<SessionMessage>,
    assigned_color: Color,
}

