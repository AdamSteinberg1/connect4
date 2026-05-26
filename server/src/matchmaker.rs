use crate::session::{Session};
use crate::{SessionInfo, SessionMessage};
use shared::{Color, JoinCode, ServerMessage};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

pub struct Matchmaker {
    rx: mpsc::Receiver<MatchmakingMessage>,
    waiting_hosts: HashMap<JoinCode, WaitingHost>,
}

pub enum MatchmakingMessage {
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

struct WaitingHost {
    session_tx: mpsc::Sender<SessionMessage>,
    session_rx: mpsc::Receiver<SessionMessage>,
    outgoing_tx: mpsc::Sender<ServerMessage>,
    color: Color,
}

impl Matchmaker {
    pub(crate) fn new(rx: mpsc::Receiver<MatchmakingMessage>) -> Self {
        Self {
            rx,
            waiting_hosts: Default::default(),
        }
    }

    pub(crate) async fn run(mut self) -> anyhow::Result<()> {
        while let Some(message) = self.rx.recv().await {
            self.handle_message(message).await?;
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: MatchmakingMessage) -> anyhow::Result<()> {
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
                let session = Session::new(host.session_rx, red_tx, yellow_tx);
                tokio::spawn(session.run());
                let _ = response_tx.send(SessionInfo {
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
