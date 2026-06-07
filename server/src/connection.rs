use crate::matchmaker::MatchmakerHandle;
use crate::session::SessionHandle;
use anyhow::anyhow;
use bytes::Bytes;
use futures::StreamExt;
use futures::sink::SinkExt;
use shared::{ClientMessage, Color, MoveError, ServerMessage};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, oneshot};
use tokio::{select, try_join};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

type Reader = FramedRead<OwnedReadHalf, LengthDelimitedCodec>;
type Writer = FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>;

enum ShutdownReason {
    ClientDisconnected,
    GameOver,
}

pub async fn handle_connection(
    stream: TcpStream,
    matchmaker: MatchmakerHandle,
) -> anyhow::Result<()> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = FramedRead::new(read_half, LengthDelimitedCodec::new());
    let mut writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());

    //this is the channel for messages that need to be sent from server to client
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<ServerMessage>(32);

    try_join!(
        handle_reads(&mut reader, &matchmaker, &outgoing_tx),
        handle_writes(&mut writer, &mut outgoing_rx),
    )?;
    Ok(())
}

async fn handle_reads(
    reader: &mut Reader,
    matchmaker: &MatchmakerHandle,
    outgoing_tx: &mpsc::Sender<ServerMessage>,
) -> anyhow::Result<()> {
    loop {
        //first we create or join a session
        let (session, color) = request_session(reader, matchmaker, outgoing_tx).await?;

        //then we relay moves to the session
        let reason = relay_moves(reader, outgoing_tx, &session, color).await?;
        if let ShutdownReason::ClientDisconnected = reason {
            return Ok(());
        }
    }
}

async fn request_session(
    reader: &mut Reader,
    matchmaker: &MatchmakerHandle,
    outgoing_tx: &mpsc::Sender<ServerMessage>,
) -> anyhow::Result<(SessionHandle, Color)> {
    while let Some(frame) = reader.next().await {
        let frame = frame?;
        let message: ClientMessage = serde_json::from_slice(&frame)?;

        match message {
            ClientMessage::CreateGame => {
                let (mut session_rx, color) = matchmaker.create_game(outgoing_tx.clone()).await?;
                let session = wait_for_session(&mut session_rx, reader, outgoing_tx).await?;
                return Ok((session, color));
            }
            ClientMessage::JoinGame { join_code } => {
                return matchmaker.join_game(join_code, outgoing_tx.clone()).await;
            }
            ClientMessage::PlayMove { .. } => {
                outgoing_tx
                    .send(ServerMessage::InvalidMove(MoveError::NotInGame))
                    .await?;
            }
        };
    }
    Err(anyhow!("disconnected"))
}

async fn wait_for_session(
    session_rx: &mut oneshot::Receiver<SessionHandle>,
    reader: &mut Reader,
    outgoing_tx: &mpsc::Sender<ServerMessage>,
) -> anyhow::Result<SessionHandle> {
    // after a client creates a game, it cannot do anything useful until an opponent joins
    // we must keep responding with GameNotStarted messages until an opponent joins
    loop {
        select! {
            session = &mut *session_rx => return Ok(session?),
            result = reader.next() => match result {
                Some(_) => outgoing_tx.send(ServerMessage::GameNotStarted).await?,
                None => return Err(anyhow!("client disconnected while waiting for opponent")),
            }
        }
    }
}

async fn relay_moves(
    reader: &mut Reader,
    outgoing_tx: &mpsc::Sender<ServerMessage>,
    session: &SessionHandle,
    color: Color,
) -> anyhow::Result<ShutdownReason> {
    loop {
        let frame = select! {
            frame = reader.next() => frame,
            _ = session.closed() => return Ok(ShutdownReason::GameOver),
        };

        let Some(frame) = frame else {
            session.player_disconnected().await?;
            return Ok(ShutdownReason::ClientDisconnected);
        };

        let frame = frame?;
        let message: ClientMessage = serde_json::from_slice(&frame)?;
        if let ClientMessage::PlayMove { column } = message {
            session.play_move(color, column).await?;
        } else {
            // client tried to create or join game after the game is already started
            outgoing_tx.send(ServerMessage::GameAlreadyStarted).await?;
        };
    }
}

async fn handle_writes(
    writer: &mut Writer,
    outgoing_rx: &mut mpsc::Receiver<ServerMessage>,
) -> anyhow::Result<()> {
    while let Some(message) = outgoing_rx.recv().await {
        let bytes = Bytes::from(serde_json::to_vec(&message)?);
        writer.send(bytes).await?;
    }
    Ok(())
}
