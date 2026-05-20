use anyhow::{Context, Result, bail};
use bytes::Bytes;
use futures::StreamExt;
use futures::sink::SinkExt;
use shared::{ClientMessage, ColumnIndex, JoinCode, ServerMessage};
use std::str::FromStr;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

const HELP: &str = "\
Commands:
  create              Send CreateGame
  join <CODE>         Send JoinGame with the given 6-char code
  play <1-7>          Send PlayMove for the given column (1-indexed)
  help                Print this message
  quit / exit         Disconnect and exit
";

#[tokio::main]
async fn main() -> Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    println!("Connecting to {addr}...");
    let stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("failed to connect to {addr}"))?;
    println!("Connected. Type 'help' for available commands.\n");

    let (read_half, write_half) = stream.into_split();

    // Channel: main loop → writer task
    let (tx, mut rx) = mpsc::channel::<ClientMessage>(16);

    // Spawn a task that reads ServerMessages and prints them
    tokio::spawn(async move {
        let mut reader = FramedRead::new(read_half, LengthDelimitedCodec::new());
        while let Some(frame) = reader.next().await {
            match frame {
                Err(e) => {
                    eprintln!("[connection error] {e}");
                    break;
                }
                Ok(bytes) => match serde_json::from_slice::<ServerMessage>(&bytes) {
                    Err(e) => eprintln!("[decode error] {e}"),
                    Ok(msg) => println!("  ← {msg:?}"),
                },
            }
        }
        println!("[server closed the connection]");
        std::process::exit(0);
    });

    // Spawn a task that drains the mpsc channel and writes to the TCP stream
    tokio::spawn(async move {
        let mut writer = FramedWrite::new(write_half, LengthDelimitedCodec::new());
        while let Some(msg) = rx.recv().await {
            let bytes = match serde_json::to_vec(&msg) {
                Ok(b) => Bytes::from(b),
                Err(e) => {
                    eprintln!("[encode error] {e}");
                    continue;
                }
            };
            if let Err(e) = writer.send(bytes).await {
                eprintln!("[write error] {e}");
                break;
            }
        }
    });

    // Main loop: read lines from stdin and parse commands
    let stdin = BufReader::new(stdin());
    let mut lines = stdin.lines();

    loop {
        print!("> ");
        // flush the prompt (stdout may be line-buffered)
        use std::io::Write;
        std::io::stdout().flush().ok();

        let line = match lines.next_line().await? {
            Some(l) => l,
            None => break, // EOF
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match parse_command(line) {
            Err(e) => {
                let msg = e.to_string();
                if !msg.is_empty() {
                    eprintln!("  error: {msg}");
                }
            }
            Ok(None) => break, // quit
            Ok(Some(msg)) => {
                println!("  → {msg:?}");
                if tx.send(msg).await.is_err() {
                    eprintln!("  [writer task gone]");
                    break;
                }
            }
        }
    }

    println!("Bye.");
    Ok(())
}

/// Returns Ok(None) on quit/exit, Ok(Some(msg)) on a valid command,
/// Err on a bad command.
fn parse_command(line: &str) -> Result<Option<ClientMessage>> {
    let mut parts = line.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("").to_ascii_lowercase();
    let rest = parts.next().unwrap_or("").trim();

    match cmd.as_str() {
        "help" => {
            print!("{HELP}");
            // Return a dummy Ok so the loop continues without sending anything.
            // We use a little trick: return early with a "no message" sentinel.
            // We can't return Ok(None) (that means quit), so we print and loop.
            // Re-use Err with a sentinel — actually cleanest to just print here:
            // Already printed above; signal caller to skip by… hmm.
            // Actually, let's just handle this by returning a special error that
            // the caller ignores. Simplest: make parse_command return a tri-state.
            // But to keep it simple, we return Ok with a fake no-op message approach.
            // Revisit: easiest is a dedicated enum. For simplicity just bail with
            // a message we suppress.
            bail!("") // caller suppresses empty-message errors for "help"
        }
        "quit" | "exit" => Ok(None),
        "create" => Ok(Some(ClientMessage::CreateGame)),
        "join" => {
            if rest.is_empty() {
                bail!("usage: join <CODE>");
            }
            let code = JoinCode::from_str(rest)
                .map_err(|_| anyhow::anyhow!("invalid join code '{rest}' (must be 6 chars from ABCDEFGHJKLMNPQRSTUVWXYZ23456789)"))?;
            Ok(Some(ClientMessage::JoinGame { join_code: code }))
        }
        "play" => {
            if rest.is_empty() {
                bail!("usage: play <1-7>");
            }
            let n: usize = rest
                .parse::<usize>()
                .with_context(|| format!("'{rest}' is not a number"))?;
            if n < 1 || n > 7 {
                bail!("column must be between 1 and 7");
            }
            let col = ColumnIndex::new(n - 1) // convert to 0-indexed
                .map_err(|_| anyhow::anyhow!("invalid column index"))?;
            Ok(Some(ClientMessage::PlayMove { column: col }))
        }
        other => bail!("unknown command '{other}' — type 'help' for options"),
    }
}