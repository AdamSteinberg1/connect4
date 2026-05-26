use crate::matchmaker::MatchmakingMessage;
use crate::{SessionInfo, SessionMessage};
use anyhow::anyhow;
use bytes::Bytes;
use futures::StreamExt;
use futures::sink::SinkExt;
use shared::{ClientMessage, MoveError, ServerMessage};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, oneshot};
use tokio::{select, try_join};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

enum ReadExit {
    ClientDisconnected,
    GameOver,
}

pub struct Connection {
    reader: FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
    writer: FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
    matchmaker_tx: mpsc::Sender<MatchmakingMessage>,
}

impl Connection {
    pub fn new(stream: TcpStream, matchmaker_tx: mpsc::Sender<MatchmakingMessage>) -> Self {
        let (read_half, write_half) = stream.into_split();

        let reader = FramedRead::new(read_half, LengthDelimitedCodec::new());
        let writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());

        Self {
            reader,
            writer,
            matchmaker_tx,
        }
    }

    pub(crate) async fn process(mut self) -> anyhow::Result<()> {
        //the channel for messages going from server to client
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<ServerMessage>(32);

        try_join!(
            Self::handle_reads(&mut self.reader, self.matchmaker_tx, outgoing_tx),
            Self::handle_writes(&mut self.writer, outgoing_rx)
        )?;
        Ok(())
    }

    async fn request_session(
        reader: &mut FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
        tx: &mpsc::Sender<MatchmakingMessage>,
        outgoing_tx: &mpsc::Sender<ServerMessage>,
    ) -> anyhow::Result<SessionInfo> {
        while let Some(frame) = reader.next().await {
            let frame = frame?;
            let message: ClientMessage = serde_json::from_slice(&frame)?;
            let (response_tx, response_rx) = oneshot::channel();
            let message = match message {
                ClientMessage::CreateGame => MatchmakingMessage::CreateGame {
                    response_tx,
                    outgoing_tx: outgoing_tx.clone(),
                },
                ClientMessage::JoinGame { join_code } => MatchmakingMessage::JoinGame {
                    response_tx,
                    outgoing_tx: outgoing_tx.clone(),
                    join_code,
                },
                ClientMessage::PlayMove { .. } => {
                    outgoing_tx
                        .send(ServerMessage::InvalidMove(MoveError::NotInGame))
                        .await?;
                    continue;
                }
            };

            //send a message to the matchmaker
            tx.send(message).await?;

            let session_info = response_rx.await?;
            return Ok(session_info);
        }
        Err(anyhow!("disconnected"))
    }

    async fn handle_reads(
        reader: &mut FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
        matchmaker_tx: mpsc::Sender<MatchmakingMessage>,
        outgoing_tx: mpsc::Sender<ServerMessage>,
    ) -> anyhow::Result<()> {
        loop {
            let session_info = Self::request_session(reader, &matchmaker_tx, &outgoing_tx).await?;

            if let ReadExit::ClientDisconnected =
                Self::relay_moves(reader, &outgoing_tx, &session_info).await?
            {
                return Ok(());
            }
        }
    }

    async fn relay_moves(
        reader: &mut FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
        outgoing_tx: &mpsc::Sender<ServerMessage>,
        session_info: &SessionInfo,
    ) -> anyhow::Result<ReadExit> {
        loop {
            // we're checking to see what happens first:
            // 1. We receive a message from the client
            // 2. The session is ended (early return)
            let frame = select! {
                frame = reader.next() => frame,
                _ = session_info.session_tx.closed() => return Ok(ReadExit::GameOver),
            };

            let Some(frame) = frame else {
                //no more messages to receive from client because it disconnected

                //we need to tell the session that a player disconnected
                session_info
                    .session_tx
                    .send(SessionMessage::PlayerDisconnected)
                    .await?;

                return Ok(ReadExit::ClientDisconnected);
            };
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
            let message = SessionMessage::MoveMessage {
                column,
                color: session_info.assigned_color,
            };
            session_info.session_tx.send(message).await?;
        }
    }

    async fn handle_writes(
        writer: &mut FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
        mut outgoing_rx: mpsc::Receiver<ServerMessage>,
    ) -> anyhow::Result<()> {
        while let Some(message) = outgoing_rx.recv().await {
            let bytes = Bytes::from(serde_json::to_vec(&message)?);
            writer.send(bytes).await?;
        }

        Ok(())
    }
}
