use anyhow::Result;
use bytes::Bytes;
use futures::sink::SinkExt;
use futures::StreamExt;
use shared::{Board, ClientMessage, Color, ColumnIndex, JoinCode, ServerMessage};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::error::SendError;
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


struct Session {
    board: Board,
    host: Player,
    guest: Option<Player>, //todo it would be real nice if this wasn't an option
}

impl Session {
    fn new(host: Player) -> Self {
        Self {
            host,
            board: Board::new(),
            guest: None,
        }
    }

    fn add_guest(&mut self, guest: Player) {
        self.guest = Some(guest);
    }

    fn get_player(&self, addr: &SocketAddr) -> Option<&Player> {
        if self.host.addr == *addr {
            return Some(&self.host);
        }
        self.guest.as_ref().filter(|guest| guest.addr == *addr)
    }

    async fn send_to_all(&mut self, msg: ServerMessage) -> Result<(), SendError<ServerMessage>> {
        //todo maybe do a join
        self.host.response_tx.send(msg.clone()).await?;
        if let Some(guest) = self.guest.as_ref() {
            guest.response_tx.send(msg).await?;
        }
        Ok(())
    }
}

struct Player {
    addr: SocketAddr,
    response_tx: Sender<ServerMessage>,
    color: Color,
}

async fn manage_sessions(mut rx: Receiver<IncomingMessage>) -> Result<()> {
    let mut sessions: HashMap<JoinCode, Session> = HashMap::new();
    let mut join_codes: HashMap<SocketAddr, JoinCode> = HashMap::new();

    while let Some(message) = rx.recv().await {
        println!("received a message: {:?}", message);
        let IncomingMessage {
            content: message,
            sender: addr,
            response_tx,
        } = message;

        match message {
            ClientMessage::CreateGame => {
                create_session(&mut sessions, &mut join_codes, addr, response_tx).await?
            }
            ClientMessage::JoinGame { code } => {
                join_session(&mut sessions, &mut join_codes, addr, response_tx, code).await?
            }
            ClientMessage::PlayMove { column } => {
                play_move(&mut sessions, &mut join_codes, addr, response_tx, column).await?
            }
        }
    }

    Ok(())
}

async fn play_move(
    sessions: &mut HashMap<JoinCode, Session>,
    join_codes: &mut HashMap<SocketAddr, JoinCode>,
    addr: SocketAddr,
    response_tx: Sender<ServerMessage>,
    column: ColumnIndex,
) -> Result<()> {
    let session = join_codes.get(&addr).and_then(|c| sessions.get_mut(c));
    let Some(session) = session else {
        let response = ServerMessage::GameNotFound; //todo is this right? This would mean the client tried to play a move without first creating or joining a game
        response_tx.send(response).await?;
        return Ok(());
    };
    let Some(player) = session.get_player(&addr) else {
        eprintln!(
            "this client played a move without joining the session. This shouldn't be possible."
        );
        return Ok(());
    };
    let color = player.color;
    if let Err(e) = session.board.play_turn(column, color) {
        let response = ServerMessage::InvalidMove(e);
        response_tx.send(response).await?;
        return Ok(());
    }

    let response = ServerMessage::MovePlayed {
        column,
        color,
        board: session.board.clone(), //todo think about if we need the board
    };
    session.send_to_all(response).await?;


    //handle game over
    if let Some(winner) = session.board.get_winner() {
        let response = ServerMessage::GameOver {
            winner: Some(winner),
        };
        session.send_to_all(response).await?;
    } else if session.board.is_full() {
        let response = ServerMessage::GameOver { winner: None };
        session.send_to_all(response).await?;
    }
    Ok(())
}

async fn join_session(
    sessions: &mut HashMap<JoinCode, Session>,
    join_codes: &mut HashMap<SocketAddr, JoinCode>,
    addr: SocketAddr,
    response_tx: Sender<ServerMessage>,
    code: JoinCode,
) -> Result<()> {
    let Some(session) = sessions.get_mut(&code) else {
        let response = ServerMessage::GameNotFound;
        response_tx.send(response).await?;
        return Ok(());
    };
    if session.host.addr == addr {
        let response = ServerMessage::CannotJoinOwnGame;
        response_tx.send(response).await?;
        return Ok(());
    }
    if session.guest.is_some() {
        let response = ServerMessage::GameFull;
        response_tx.send(response).await?;
        return Ok(());
    }

    let guest = Player {
        addr,
        response_tx,
        color: Color::Yellow, //the game guest will always be yellow
    };
    session.add_guest(guest);
    join_codes.insert(addr, code);

    let host_response = ServerMessage::GameStarted {
        your_color: session.host.color,
    };
    session.host.response_tx.send(host_response).await?;

    let guest_response = ServerMessage::GameStarted {
        your_color: session.guest.as_ref().unwrap().color,
    };
    session
        .guest
        .as_ref()
        .unwrap()
        .response_tx
        .send(guest_response)
        .await?;
    Ok(())
}

async fn create_session(
    sessions: &mut HashMap<JoinCode, Session>,
    join_codes: &mut HashMap<SocketAddr, JoinCode>,
    addr: SocketAddr,
    response_tx: Sender<ServerMessage>,
) -> Result<()> {
    let join_code = unused_join_code(&sessions);
    let response = ServerMessage::GameCreated { join_code };
    response_tx.send(response).await?;
    let host = Player {
        addr,
        response_tx,
        //the game creator will always be the red player
        //todo maybe randomize this
        color: Color::Red,
    };
    let session = Session::new(host);
    sessions.insert(join_code, session);
    join_codes.insert(addr, join_code);
    Ok(())
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
