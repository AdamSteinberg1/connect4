use anyhow::{Result, anyhow, bail};
use bytes::Bytes;
use futures::StreamExt;
use futures::sink::SinkExt;
use shared::Color::Red;
use shared::{Board, ClientMessage, Color, ColumnIndex, JoinCode, MoveError, ServerMessage};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::join;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
use tokio::try_join;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = tokio::sync::mpsc::channel::<MatchmakingMessage>(32);

    let session_manager = matchmake(rx);
    let connection_handler = handle_connections(tx);
    try_join!(session_manager, connection_handler)?;
    Ok(())
}

struct WaitingHost {
    session_tx: mpsc::Sender<MoveMessage>,
    session_rx: mpsc::Receiver<MoveMessage>,
    color: Color,
}

async fn handle_connections(tx: mpsc::Sender<MatchmakingMessage>) -> Result<()> {
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
    tx: mpsc::Sender<MatchmakingMessage>,
) -> Result<()> {
    let (read_half, write_half) = stream.into_split();

    //the channel for messages going from server to client
    let (response_tx, response_rx) = mpsc::channel::<ServerMessage>(32);

    try_join!(
        read_stream(addr, tx, response_tx, read_half),
        write_stream(response_rx, write_half)
    )?;

    println!("{addr} disconnected"); //todo we need to end the session
    Ok(())
}

async fn read_stream(
    addr: SocketAddr,
    tx: mpsc::Sender<MatchmakingMessage>,
    player_tx: mpsc::Sender<ServerMessage>,
    read_half: OwnedReadHalf,
) -> Result<()> {
    let mut reader = FramedRead::new(read_half, LengthDelimitedCodec::new());

    let session_info = loop {
        let Some(frame) = reader.next().await else {
            return Ok(()); //client disconnected
        };
        let frame = frame?;
        let message: ClientMessage = serde_json::from_slice(&frame)?;
        let (response_tx, response_rx) = oneshot::channel();
        let message = match message {
            ClientMessage::CreateGame => MatchmakingMessage::CreateGame { response_tx },
            ClientMessage::JoinGame { join_code } => MatchmakingMessage::JoinGame {
                response_tx,
                join_code,
            },
            ClientMessage::PlayMove { .. } => {
                //ignore for now, eventually write back error
                continue;
            }
        };

        //send a message to the matchmaker
        tx.send(message).await?;

        // the channel that our matchmaker has made for us
        // this is where we send moves to
        let session_tx = response_rx.await?;
        break session_tx;
    };

    //todo tell the client what the join code is

    while let Some(frame) = reader.next().await {
        let frame = frame?;
        let message: ClientMessage = serde_json::from_slice(&frame)?;
        let ClientMessage::PlayMove { column } = message else {
            //for now ignore other message types
            //eventually respond with an error
            continue;
        };
        let message = MoveMessage { column, addr };
        session_info.session_tx.send(message).await?;
    }
    Ok(())
}

async fn write_stream(
    mut rx: mpsc::Receiver<ServerMessage>,
    write_half: OwnedWriteHalf,
) -> Result<()> {
    let mut writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());

    while let Some(message) = rx.recv().await {
        let bytes = Bytes::from(serde_json::to_vec(&message)?);
        writer.send(bytes).await?;
    }

    Ok(())
}

enum MatchmakingMessage {
    JoinGame {
        response_tx: oneshot::Sender<SessionInfo>,
        join_code: JoinCode,
    },
    CreateGame {
        response_tx: oneshot::Sender<SessionInfo>,
    },
}

#[derive(Debug)]
struct SessionInfo {
    join_code: JoinCode,
    session_tx: mpsc::Sender<MoveMessage>,
    assigned_color: Color
}

struct MoveMessage {
    color: Color,
    column: ColumnIndex,
}

struct Session {
    rx: mpsc::Receiver<MoveMessage>, // we receive moves from both players on one channel
    red_tx: mpsc::Sender<ServerMessage>,
    yellow_tx: mpsc::Sender<ServerMessage>,
    board: Board,
}

impl Session {
    async fn run(&mut self) -> Result<()> {
        while let Some(message) = self.rx.recv().await {
            self.handle_message(message).await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: MoveMessage) -> Result<()> {
        let MoveMessage { color, column } = message;
        match self.board.play_turn(column, color) {
            Ok(_) => {
                let response = ServerMessage::MovePlayed {
                    column,
                    color,
                    board: self.board.clone(),
                };
                let other_player_tx = match color {
                    Color::Yellow => &self.red_tx,
                    Color::Red => &self.yellow_tx,
                };
                other_player_tx.send(response).await?;
            }
            Err(e) => {
                let response = ServerMessage::InvalidMove(e);
                let response_tx = match color {
                    Color::Red => &self.red_tx,
                    Color::Yellow => &self.yellow_tx,
                };
                response_tx.send(response).await?;
            }
        }
        Ok(())
    }
}

struct MatchMaker {
    rx: mpsc::Receiver<MatchmakingMessage>,
    waiting_hosts: HashMap<JoinCode, WaitingHost>,
}

impl MatchMaker {
    async fn run(&mut self) -> Result<()> {
        while let Some(message) = self.rx.recv().await {
            self.handle_message(message).await;
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: MatchmakingMessage) {
        match message {
            MatchmakingMessage::CreateGame { response_tx } => {
                let (session_tx, session_rx) = mpsc::channel(32);
                let 
                let host = WaitingHost {
                    session_tx: session_tx.clone(),
                    session_rx,
                };
                let join_code = self.unused_join_code();
                self.waiting_hosts.insert(join_code, host);
                let _ = response_tx.send(SessionInfo {
                    join_code,
                    session_tx,
                });
            }
            MatchmakingMessage::JoinGame {
                join_code,
                response_tx,
            } => {
                //join existing session
                let host = self.waiting_hosts.remove(&join_code).unwrap();
                let mut session = Session {
                    rx: host.session_rx,
                    red_tx: todo!(),
                    yellow_tx: todo!(),
                    board: Default::default(),
                };
                tokio::spawn(session.run());
                let _ = response_tx.send(SessionInfo {
                    join_code,
                    session_tx: host.session_tx,
                });
            }
        }
    }
    fn unused_join_code(&self) -> JoinCode {
        loop {
            let code = JoinCode::random();
            if !self.waiting_hosts.contains_key(&code) {
                return code;
            }
        }
    }
}

struct Connection {
    reader: FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
    writer: FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();

        let reader = FramedRead::new(read_half, LengthDelimitedCodec::new());
        let writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());

        Self { reader, writer }
    }

    async fn process(&mut self, matchmaker_tx: mpsc::Sender<MatchmakingMessage>) -> Result<()> {
        //the channel for messages going from server to client
        let (response_tx, response_rx) = mpsc::channel::<ServerMessage>(32);

        try_join!(
            self.handle_reads(matchmaker_tx, response_tx),
            self.handle_writes(response_rx)
        )?;

        println!("disconnected"); //todo we need to end the session
        Ok(())
    }

    async fn request_session(
        &mut self,
        tx: mpsc::Sender<MatchmakingMessage>,
    ) -> Result<SessionInfo> {
        while let Some(frame) = self.reader.next().await {
            let frame = frame?;
            let message: ClientMessage = serde_json::from_slice(&frame)?;
            let (response_tx, response_rx) = oneshot::channel();
            let message = match message {
                ClientMessage::CreateGame => MatchmakingMessage::CreateGame { response_tx },
                ClientMessage::JoinGame { join_code } => MatchmakingMessage::JoinGame {
                    response_tx,
                    join_code,
                },
                ClientMessage::PlayMove { .. } => {
                    //ignore for now, eventually write back error
                    continue;
                }
            };

            //send a message to the matchmaker
            tx.send(message).await?;

            // the channel that our matchmaker has made for us
            // this is where we send moves to
            let session_tx = response_rx.await?;
            return Ok(session_tx);
        }
        Err(anyhow!("disconnected"))
    }

    async fn handle_reads(
        &mut self,
        matchmaker_tx: mpsc::Sender<MatchmakingMessage>,
        response_tx: mpsc::Sender<ServerMessage>,
    ) -> Result<()> {
        let session_info = self.request_session(matchmaker_tx).await?;

        //todo tell the client what the join code is

        while let Some(frame) = self.reader.next().await {
            let frame = frame?;
            let message: ClientMessage = serde_json::from_slice(&frame)?;
            let ClientMessage::PlayMove { column } = message else {
                //for now ignore other message types
                //eventually respond with an error
                continue;
            };
            //todo figure out how to determine color
            let message = MoveMessage { column, color: Red };
            session_info.session_tx.send(message).await?;
        }
        Ok(())
    }

    async fn handle_writes(&self, response_rx: mpsc::Receiver<ServerMessage>) -> Result<()> {
        Ok(())
    }
}
