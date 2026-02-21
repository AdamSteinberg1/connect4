use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Listening on port 8080...");

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("New connection from {addr}");

        tokio::spawn(async move {
            let reader = BufReader::new(stream);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                println!("[{addr}]: {line}");
            }

            println!("{addr} disconnected");
        });
    }
}