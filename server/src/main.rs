use anyhow::Result;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::sync::broadcast::{Receiver, Sender};


//TODO right now every message is broadcast to all connections
// We want messages to only be broadcast to clients in the same "room"

#[tokio::main]
async fn main() -> Result<()> {

    let (broadcast_tx, _) = tokio::sync::broadcast::channel::<String>(32);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Listening on port 8080...");

    loop {
        let (stream, addr) = listener.accept().await?;
        let broadcast_tx = broadcast_tx.clone();
        let broadcast_rx = broadcast_tx.subscribe();

        let (read_half, write_half) = stream.into_split();

        tokio::spawn(async move {
            if let Err(e) = handle_reader(addr, broadcast_tx, read_half).await {
                println!("Error while handling reader: {}", e);
            }
        });

        tokio::spawn(async move {
            if let Err(e) = handle_writer(broadcast_rx, write_half).await {
                println!("Error while handling writer: {}", e);
            }
        });
    }
}

async fn handle_writer(mut broadcast_rx: Receiver<String>, mut write_half: OwnedWriteHalf) -> Result<()> {
    while let Ok(message) = broadcast_rx.recv().await {
        write_half.write_all(message.as_bytes()).await?;
    }
    Ok(())

}

async fn handle_reader(addr: SocketAddr, broadcast_tx: Sender<String>, read_half: OwnedReadHalf) -> Result<()> {
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        let message = format!("{addr}: {line}\n");
        print!("{message}");
        broadcast_tx.send(message).map_err(|e| anyhow::anyhow!("Error sending message: {}", e))?;
    }
    Ok(())
}
