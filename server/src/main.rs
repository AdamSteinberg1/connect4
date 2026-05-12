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
    let connection_handler = handle_connections(tx);
    try_join!(session_manager, connection_handler)?;
    Ok(())
}

struct Session {
    board: Board,
    host: Player,
    guest: Player,
}

impl Session {
    fn new(host: Player, guest: Player) -> Self {
        Self {
            host,
            guest,
            board: Board::new(),
        }
    }

    fn get_player(&self, addr: &SocketAddr) -> Option<&Player> {
        if self.host.addr == *addr {
            Some(&self.host)
        } else if self.guest.addr == *addr {
            Some(&self.guest)
        } else {
            None
        }
    }

    async fn send_to_all(&mut self, msg: ServerMessage) -> Result<(), SendError<ServerMessage>> {
        let host_future = self.host.response_tx.send(msg.clone());
        let guest_future = self.guest.response_tx.send(msg);
        try_join!(host_future,guest_future)?;
        Ok(())
    }
}

struct Player {
    addr: SocketAddr,
    response_tx: Sender<ServerMessage>,
    color: Color,
}

async fn manage_sessions(mut rx: Receiver<IncomingMessage>) -> Result<()> {
    //players that have created a room and are waiting for someone to join
    let mut waiting_players: HashMap<JoinCode, Player> = HashMap::new();
    let mut sessions: HashMap<JoinCode, Session> = HashMap::new();
    let mut join_codes: HashMap<SocketAddr, JoinCode> = HashMap::new();

    while let Some(message) = rx.recv().await {
        let IncomingMessage {
            content: message,
            sender: addr,
            response_tx,
        } = message;

        match message {
            ClientMessage::CreateGame => {
                register_game(&mut waiting_players, &mut join_codes, addr, response_tx).await?
            }
            ClientMessage::JoinGame { code } => {
                join_session(
                    &mut waiting_players,
                    &mut sessions,
                    &mut join_codes,
                    addr,
                    response_tx,
                    code,
                )
                .await?
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
    let Some(player) = session.get_player(&addr) else { //todo I'd like for this check to not be necessary
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
    let game_over_response = if let Some(winner) = session.board.get_winner() {
        Some(ServerMessage::GameOver {
            winner: Some(winner),
        })
    } else if session.board.is_full() {
        Some(ServerMessage::GameOver { winner: None })
    } else {
        None
    };
    if let Some(response) = game_over_response {
        let join_code_1 = join_codes.remove(&session.host.addr).unwrap();
        let join_code_2 = join_codes.remove(&session.guest.addr).unwrap();
        assert_eq!(join_code_1, join_code_2); //todo
        let mut session = sessions.remove(&join_code_1).unwrap();
        session.send_to_all(response).await?;
    }
    Ok(())
}

async fn join_session(
    waiting_players: &mut HashMap<JoinCode, Player>,
    sessions: &mut HashMap<JoinCode, Session>,
    join_codes: &mut HashMap<SocketAddr, JoinCode>,
    addr: SocketAddr,
    response_tx: Sender<ServerMessage>,
    code: JoinCode,
) -> Result<()> {
    //check if the session already exists
    if sessions.contains_key(&code) {
        let response = ServerMessage::GameFull;
        response_tx.send(response).await?;
        return Ok(());
    }

    //check if the host is trying to join their own session
    if waiting_players
        .get(&code)
        .is_some_and(|host| host.addr == addr)
    {
        let response = ServerMessage::CannotJoinOwnGame;
        response_tx.send(response).await?;
        return Ok(());
    }

    //check if a host is waiting
    let Some(host) = waiting_players.remove(&code) else {
        let response = ServerMessage::GameNotFound;
        response_tx.send(response).await?;
        return Ok(());
    };

    let guest = Player {
        addr,
        response_tx,
        color: Color::Yellow, //the game guest will always be yellow
    };

    let session = Session::new(host, guest);
    let host_response = ServerMessage::GameStarted {
        your_color: session.host.color,
    };
    session.host.response_tx.send(host_response).await?;

    let guest_response = ServerMessage::GameStarted {
        your_color: session.guest.color,
    };
    session.guest.response_tx.send(guest_response).await?;

    sessions.insert(code, session);
    join_codes.insert(addr, code);
    Ok(())
}

async fn register_game(
    waiting_players: &mut HashMap<JoinCode, Player>,
    join_codes: &mut HashMap<SocketAddr, JoinCode>,
    addr: SocketAddr,
    response_tx: Sender<ServerMessage>,
) -> Result<()> {
    let join_code = unused_join_code(|k| waiting_players.contains_key(k));
    let response = ServerMessage::GameCreated { join_code };
    response_tx.send(response).await?;
    let host = Player {
        addr,
        response_tx,
        //the game creator will always be the red player
        //todo maybe randomize this
        color: Color::Red,
    };
    waiting_players.insert(join_code, host);
    join_codes.insert(addr, join_code);
    Ok(())
}

fn unused_join_code(contains_key: impl Fn(&JoinCode) -> bool) -> JoinCode {
    loop {
        let join_code = JoinCode::random();
        if !contains_key(&join_code) {
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
