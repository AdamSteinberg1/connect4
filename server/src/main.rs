mod connection;
mod matchmaker;
mod session;

use crate::connection::handle_connection;
use crate::matchmaker::MatchmakerHandle;
use anyhow::Result;
use futures::TryStreamExt;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;

#[tokio::main]
async fn main() -> Result<()> {
    let matchmaker = MatchmakerHandle::new();
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Listening on port 8080...");

    TcpListenerStream::new(listener)
        .map_err(anyhow::Error::from)
        .try_for_each_concurrent(None, async |stream| {
            let matchmaker = matchmaker.clone();
            handle_connection(stream, matchmaker).await
        })
        .await
}
