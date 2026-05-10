use anyhow::Result;
use bytes::Bytes;
use futures::StreamExt;
use futures::sink::SinkExt;
use shared::{Board, ClientMessage, Color, ColumnIndex, JoinCode, ServerMessage};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::try_join;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = tokio::sync::mpsc::channel::<IncomingMessage>(32);

    let session_manager = manage_sessions(rx);
    let connection_handler = handle_connections(tx); //todo handle error
    try_join!(session_manager, connection_handler)?;
    Ok(())
}

#[derive(Default)]
struct Session {
    board: Board,
    red_addr: Option<SocketAddr>,
    yellow_addr: Option<SocketAddr>,
}

async fn manage_sessions(mut rx: Receiver<IncomingMessage>) -> Result<()> {
    let mut sessions: HashMap<JoinCode, Session> = HashMap::new();
    let mut join_codes: HashMap<SocketAddr, JoinCode> = HashMap::new();

    loop {
        let Some(message) = rx.recv().await else {
            return Ok(());
        };
        println!("received a message: {:?}", message);
        let IncomingMessage {
            content: message,
            sender: addr,
            response_tx,
        } = message;

        match message {
            ClientMessage::CreateGame => {
                let join_code = unused_join_code(&sessions);
                let mut session = Session::default();
                session.red_addr = Some(addr); //the game creator will always be the red player
                sessions.insert(join_code, session);
                join_codes.insert(addr, join_code);
                let response = ServerMessage::GameCreated { code: join_code }; //todo think about how to notify creator that someone joined
                response_tx.send(response).await?;
                continue;
            }
            ClientMessage::JoinGame { code } => {
                if let Some(session) = sessions.get_mut(&code) {
                    if session.yellow_addr.is_some() {
                        eprintln!("This game room is full.");
                        let response = ServerMessage::GameFull;
                        response_tx.send(response).await?;
                        continue;
                    }
                    session.yellow_addr = Some(addr); //the game joiner will always be yellow
                    join_codes.insert(addr, code);
                    let response = ServerMessage::GameStarted {
                        your_color: Color::Yellow,
                    };
                    response_tx.send(response).await?;
                    continue;
                } else {
                    let response = ServerMessage::GameNotFound;
                    response_tx.send(response).await?;
                    continue;
                }
            }
            ClientMessage::PlayMove { column } => {
                let session = join_codes.get(&addr).and_then(|c| sessions.get_mut(c));
                let Some(session) = session else {
                    let response = ServerMessage::GameNotFound; //todo is this right? This would mean the client tried to play a move without first creating or joining a game
                    response_tx.send(response).await?;
                    continue;
                };
                let color = if Some(addr) == session.red_addr {
                    Color::Red
                } else if Some(addr) == session.yellow_addr {
                    Color::Yellow
                } else {
                    eprintln!(
                        "Error you tried to play in a game you haven't joined. This shouldn't be possible"
                    );
                    continue;
                };
                if let Err(e) = session.board.play_turn(column, color) {
                    let response = ServerMessage::InvalidMove(e);
                    response_tx.send(response).await?;
                    continue;
                }

                if let Some(winner) = session.board.get_winner() {
                    let response = ServerMessage::GameOver {
                        winner: Some(winner),
                    };
                    response_tx.send(response).await?;
                    continue;
                }

                if session.board.is_full() {
                    let response = ServerMessage::GameOver { winner: None };
                    response_tx.send(response).await?;
                    continue;
                }
            }
        }
    }
}

fn unused_join_code(sessions: &HashMap<JoinCode, Session>) -> JoinCode {
    loop {
        let join_code = JoinCode::random();
        if !sessions.contains_key(&join_code) {
            return join_code;
        }
    }
}

async fn handle_connections(tx: Sender<IncomingMessage>) -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Listening on port 8080...");

    loop {
        let (stream, addr) = listener.accept().await?;
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_stream(stream, addr, tx).await {
                eprintln!("connection {addr} failed: {e:#}");
            }
        });
    }
}

async fn handle_stream(
    stream: TcpStream,
    addr: SocketAddr,
    tx: Sender<IncomingMessage>,
) -> Result<()> {
    let (read_half, write_half) = stream.into_split();

    let (response_tx, response_rx) = tokio::sync::mpsc::channel::<ServerMessage>(32);

    try_join!(
        read_stream(addr, tx, response_tx, read_half),
        write_stream(response_rx, write_half)
    )?;

    println!("{addr} disconnected"); //todo we need to end the session
    Ok(())
}

async fn read_stream(
    addr: SocketAddr,
    tx: Sender<IncomingMessage>,
    response_tx: Sender<ServerMessage>,
    read_half: OwnedReadHalf,
) -> Result<()> {
    let mut reader = FramedRead::new(read_half, LengthDelimitedCodec::new());

    while let Some(frame) = reader.next().await {
        let frame = frame?;
        let message: ClientMessage = serde_json::from_slice(&frame)?;
        println!("Server received: {:?}", message);
        let message = IncomingMessage {
            content: message,
            sender: addr,
            response_tx: response_tx.clone(),
        };
        tx.send(message).await?;
    }
    Ok(())
}

async fn write_stream(mut rx: Receiver<ServerMessage>, write_half: OwnedWriteHalf) -> Result<()> {
    let mut writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());

    while let Some(message) = rx.recv().await {
        let bytes = Bytes::from(serde_json::to_vec(&message)?);
        writer.send(bytes).await?;
    }

    Ok(())
}

#[derive(Debug)]
struct IncomingMessage {
    sender: SocketAddr,
    content: ClientMessage,
    response_tx: Sender<ServerMessage>,
}
