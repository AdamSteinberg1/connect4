use anyhow::Context;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use shared::{ClientMessage, ServerMessage};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio::select;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

pub const DEFAULT_SERVER: &str = "193.122.250.100:8080";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CHANNEL_CAPACITY: usize = 32;

/// A live connection to the game server.
///
/// Dropping it closes the socket, which the server treats as the player leaving.
pub struct Connection {
    outgoing_tx: mpsc::Sender<ClientMessage>,
    incoming_rx: mpsc::Receiver<ServerMessage>,
}

impl Connection {
    pub async fn open(addr: &str) -> anyhow::Result<Self> {
        let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .context("timed out")??;
        stream.set_nodelay(true)?;

        let (outgoing_tx, outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        tokio::spawn(async move {
            // any failure ends the task, which closes `incoming_rx`.
            // the app treats that as a lost connection
            let _ = run(stream, outgoing_rx, incoming_tx).await;
        });

        Ok(Self {
            outgoing_tx,
            incoming_rx,
        })
    }

    /// queues a message for the server. If the connection is gone, the message is dropped
    /// and `recv` will report the closure
    pub fn send(&self, message: ClientMessage) {
        let _ = self.outgoing_tx.try_send(message);
    }

    /// waits for the next server message, or None once the connection has closed
    pub async fn recv(&mut self) -> Option<ServerMessage> {
        self.incoming_rx.recv().await
    }
}

async fn run(
    stream: TcpStream,
    mut outgoing_rx: mpsc::Receiver<ClientMessage>,
    incoming_tx: mpsc::Sender<ServerMessage>,
) -> anyhow::Result<()> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = FramedRead::new(read_half, LengthDelimitedCodec::new());
    let mut writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());

    // a single task owns both halves, so that when the `Connection` is dropped
    // (outgoing_rx closes) both halves drop and the socket actually closes
    loop {
        select! {
            message = outgoing_rx.recv() => {
                let Some(message) = message else { return Ok(()) };
                writer.send(Bytes::from(serde_json::to_vec(&message)?)).await?;
            }
            frame = reader.next() => {
                let Some(frame) = frame else { return Ok(()) };
                let message: ServerMessage = serde_json::from_slice(&frame?)?;
                if incoming_tx.send(message).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}
