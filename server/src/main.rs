use std::net::SocketAddr;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{OwnedWriteHalf};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    let writers: Arc<Mutex<Vec<OwnedWriteHalf>>> = Arc::new(Mutex::new(Vec::new()));

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Listening on port 8080...");

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("New connection from {addr}");

        let writers = writers.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, writers).await {
                println!("Error : {e}");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream, addr: SocketAddr, writers: Arc<Mutex<Vec<OwnedWriteHalf>>>) -> Result<(), tokio::io::Error> {
    let (reader, writer) = stream.into_split();

    writers.lock().await.push(writer);

    let reader = BufReader::new(reader);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        let message = format!("[{addr}]: {line}\n");
        print!("{}", message);
        let mut writers = writers.lock().await;
        for writer in writers.iter_mut() {
            writer.write_all(message.as_bytes()).await?;
        }
    }

    println!("{addr} disconnected");
    Ok(())
}
