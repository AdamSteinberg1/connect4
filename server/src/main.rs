use crate::SessionMessage::MoveMessage;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::future::try_join;
use futures::sink::SinkExt;
use futures::StreamExt;
use shared::{Board, ClientMessage, Color, ColumnIndex, JoinCode, ServerMessage};
use std::collections::HashMap;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::try_join;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = mpsc::channel::<MatchmakingMessage>(32);

    let matchmaker = MatchMaker::new(rx);
    let connection_handler = handle_connections(tx);
    try_join!(matchmaker.run(), connection_handler)?;
    Ok(())
}

struct WaitingHost {
    session_tx: mpsc::Sender<SessionMessage>,
    session_rx: mpsc::Receiver<SessionMessage>,
    outgoing_tx: mpsc::Sender<ServerMessage>,
    color: Color,
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

enum MatchmakingMessage {
    JoinGame {
        outgoing_tx: mpsc::Sender<ServerMessage>,
        response_tx: oneshot::Sender<SessionInfo>,
        join_code: JoinCode,
    },
    CreateGame {
        outgoing_tx: mpsc::Sender<ServerMessage>,
        response_tx: oneshot::Sender<SessionInfo>,
    },
}

#[derive(Debug)]
struct SessionInfo {
    join_code: JoinCode,
    session_tx: mpsc::Sender<SessionMessage>,
    assigned_color: Color,
}

//message sent to a session
enum SessionMessage {
    MoveMessage { color: Color, column: ColumnIndex },
    PlayerDisconnected,
}

struct Session {
    rx: mpsc::Receiver<SessionMessage>, // we receive moves from both players on one channel
    red_tx: mpsc::Sender<ServerMessage>,
    yellow_tx: mpsc::Sender<ServerMessage>,
    board: Board,
}

impl Session {
    async fn send_to_all_players(&mut self, msg: ServerMessage) -> Result<()> {
        try_join(self.red_tx.send(msg.clone()), self.yellow_tx.send(msg)).await?;
        Ok(())
    }

    async fn run(mut self) -> Result<()> {
        try_join(
            self.red_tx.send(ServerMessage::GameStarted {
                your_color: Color::Red,
            }),
            self.yellow_tx.send(ServerMessage::GameStarted {
                your_color: Color::Yellow,
            }),
        )
        .await?;

        while let Some(message) = self.rx.recv().await {
            self.handle_message(message).await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: SessionMessage) -> Result<()> {
        let (color, column) = match message {
            MoveMessage { color, column } => (color, column),
            SessionMessage::PlayerDisconnected => {
                //todo should we send to both players? One of them is disconnected
                self.send_to_all_players(ServerMessage::OpponentDisconnected)
                    .await?;
                return Ok(());
            }
        };
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
        if let Some(winner) = self.board.get_winner() {
            let msg = ServerMessage::GameOver {
                winner: Some(winner),
            };
            self.send_to_all_players(msg).await?;
        }
        if self.board.is_full() {
            self.send_to_all_players(ServerMessage::GameOver { winner: None })
                .await?;
        }

        Ok(())
    }
}

struct MatchMaker {
    rx: mpsc::Receiver<MatchmakingMessage>,
    waiting_hosts: HashMap<JoinCode, WaitingHost>,
}

impl MatchMaker {
    fn new(rx: mpsc::Receiver<MatchmakingMessage>) -> Self {
        Self {
            rx,
            waiting_hosts: Default::default(),
        }
    }

    async fn run(mut self) -> Result<()> {
        while let Some(message) = self.rx.recv().await {
            self.handle_message(message).await?;
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: MatchmakingMessage) -> Result<()> {
        match message {
            MatchmakingMessage::CreateGame {
                response_tx,
                outgoing_tx,
            } => {
                let join_code = self.unused_join_code();
                outgoing_tx
                    .send(ServerMessage::GameCreated { join_code })
                    .await?;

                let (session_tx, session_rx) = mpsc::channel(32);
                let color = rand::random();
                let host = WaitingHost {
                    session_tx: session_tx.clone(),
                    outgoing_tx,
                    color,
                    session_rx,
                };

                self.waiting_hosts.insert(join_code, host);
                let _ = response_tx.send(SessionInfo {
                    join_code,
                    session_tx,
                    assigned_color: color,
                });
            }
            MatchmakingMessage::JoinGame {
                join_code,
                response_tx,
                outgoing_tx,
            } => {
                //join existing session
                let host = self.waiting_hosts.remove(&join_code).unwrap();
                let (red_tx, yellow_tx) = match host.color {
                    Color::Red => (host.outgoing_tx, outgoing_tx),
                    Color::Yellow => (outgoing_tx, host.outgoing_tx),
                };
                let session = Session {
                    rx: host.session_rx,
                    red_tx,
                    yellow_tx,
                    board: Default::default(),
                };
                tokio::spawn(session.run());
                let _ = response_tx.send(SessionInfo {
                    join_code,
                    session_tx: host.session_tx,
                    assigned_color: host.color.other(),
                });
            }
        };
        Ok(())
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
    matchmaker_tx: mpsc::Sender<MatchmakingMessage>,
}

impl Connection {
    fn new(stream: TcpStream, matchmaker_tx: mpsc::Sender<MatchmakingMessage>) -> Self {
        let (read_half, write_half) = stream.into_split();

        let reader = FramedRead::new(read_half, LengthDelimitedCodec::new());
        let writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());

        Self {
            reader,
            writer,
            matchmaker_tx,
        }
    }

    async fn process(mut self) -> Result<()> {
        //the channel for messages going from server to client
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<ServerMessage>(32);

        try_join!(
            Self::handle_reads(&mut self.reader, self.matchmaker_tx, outgoing_tx),
            Self::handle_writes(&mut self.writer, outgoing_rx)
        )?;

        println!("disconnected"); //todo we need to end the session
        Ok(())
    }

    async fn request_session(
        reader: &mut FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
        tx: mpsc::Sender<MatchmakingMessage>,
        outgoing_tx: mpsc::Sender<ServerMessage>,
    ) -> Result<SessionInfo> {
        while let Some(frame) = reader.next().await {
            let frame = frame?;
            let message: ClientMessage = serde_json::from_slice(&frame)?;
            let (response_tx, response_rx) = oneshot::channel();
            let message = match message {
                ClientMessage::CreateGame => MatchmakingMessage::CreateGame {
                    response_tx,
                    outgoing_tx,
                },
                ClientMessage::JoinGame { join_code } => MatchmakingMessage::JoinGame {
                    response_tx,
                    outgoing_tx,
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
        reader: &mut FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
        matchmaker_tx: mpsc::Sender<MatchmakingMessage>,
        outgoing_tx: mpsc::Sender<ServerMessage>,
    ) -> Result<()> {
        let session_info =
            Self::request_session(reader, matchmaker_tx, outgoing_tx.clone()).await?;

        while let Some(frame) = reader.next().await {
            let frame = frame?;
            let message: ClientMessage = serde_json::from_slice(&frame)?;
            let ClientMessage::PlayMove { column } = message else {
                //the game has already started, so anything other than playing a move is invalid
                let message = ServerMessage::GameStarted {
                    your_color: session_info.assigned_color,
                };
                outgoing_tx.send(message).await?;
                continue;
            };
            let message = MoveMessage {
                column,
                color: session_info.assigned_color,
            };
            session_info.session_tx.send(message).await?;
        }
        session_info
            .session_tx
            .send(SessionMessage::PlayerDisconnected)
            .await?;
        Ok(())
    }

    async fn handle_writes(
        writer: &mut FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
        mut outgoing_rx: mpsc::Receiver<ServerMessage>,
    ) -> Result<()> {
        while let Some(message) = outgoing_rx.recv().await {
            let bytes = Bytes::from(serde_json::to_vec(&message)?);
            writer.send(bytes).await?;
        }

        Ok(())
    }
}
