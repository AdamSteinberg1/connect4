use crate::session::SessionHandle;
use shared::{Color, JoinCode, ServerMessage};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

struct Matchmaker {
    rx: mpsc::Receiver<MatchmakingMessage>,
    waiting_hosts: HashMap<JoinCode, WaitingHost>,
}

enum MatchmakingMessage {
    Create {
        outgoing_tx: mpsc::Sender<ServerMessage>,
        session_tx: oneshot::Sender<SessionHandle>,
        color_tx: oneshot::Sender<Color>,
        join_code_tx: oneshot::Sender<JoinCode>,
    },
    Cancel {
        join_code: JoinCode,
    },
    Join {
        outgoing_tx: mpsc::Sender<ServerMessage>,
        response_tx: oneshot::Sender<Option<(SessionHandle, Color)>>,
        join_code: JoinCode,
    },
}

// info for a game that has been created, but not yet started.
// the host is waiting for an opponent to join
struct WaitingHost {
    session_tx: oneshot::Sender<SessionHandle>,
    outgoing_tx: mpsc::Sender<ServerMessage>,
    color: Color,
}

impl Matchmaker {
    fn new(rx: mpsc::Receiver<MatchmakingMessage>) -> Self {
        Self {
            rx,
            waiting_hosts: Default::default(),
        }
    }

    //this has to consume self, because you can only run an actor once
    async fn run(self) {
        if let Err(e) = self.try_run().await {
            eprintln!("matchmaker encountered an error: {:?}", e);
        }
    }

    async fn try_run(mut self) -> anyhow::Result<()> {
        while let Some(message) = self.rx.recv().await {
            self.handle_message(message).await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: MatchmakingMessage) -> anyhow::Result<()> {
        match message {
            MatchmakingMessage::Create {
                outgoing_tx,
                session_tx,
                color_tx,
                join_code_tx,
            } => {
                let join_code = self.unused_join_code();
                outgoing_tx
                    .send(ServerMessage::GameCreated { join_code })
                    .await?;

                let color: Color = rand::random();
                let _ = color_tx.send(color);
                let _ = join_code_tx.send(join_code);
                self.waiting_hosts.insert(
                    join_code,
                    WaitingHost {
                        session_tx,
                        outgoing_tx,
                        color,
                    },
                );
            }
            MatchmakingMessage::Cancel { join_code } => {
                self.waiting_hosts.remove(&join_code);
            }
            MatchmakingMessage::Join {
                join_code,
                response_tx,
                outgoing_tx,
            } => {
                let Some(host) = self.waiting_hosts.remove(&join_code) else {
                    let _ = response_tx.send(None);
                    return Ok(());
                };

                let (red_tx, yellow_tx) = match host.color {
                    Color::Red => (host.outgoing_tx, outgoing_tx),
                    Color::Yellow => (outgoing_tx, host.outgoing_tx),
                };

                let session = SessionHandle::new(red_tx, yellow_tx);

                // deliver the SessionHandle to the waiting host connection
                let _ = host.session_tx.send(session.clone());
                let _ = response_tx.send(Some((session, host.color.other())));
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

//the handle is what actually spawns the task and handles messaging
#[derive(Clone)]
pub struct MatchmakerHandle {
    tx: mpsc::Sender<MatchmakingMessage>,
}

impl MatchmakerHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);
        let actor = Matchmaker::new(rx);
        tokio::spawn(actor.run());

        Self { tx }
    }

    // creates a new game that another player can join
    // returns a receiver that will receive a session handle once an opponent has join
    // and also color the host has been assigned
    pub async fn create_game(
        &self,
        outgoing_tx: mpsc::Sender<ServerMessage>,
    ) -> anyhow::Result<(oneshot::Receiver<SessionHandle>, Color, JoinCode)> {
        let (session_tx, session_rx) = oneshot::channel();
        let (color_tx, color_rx) = oneshot::channel();
        let (join_code_tx, join_code_rx) = oneshot::channel();
        self.tx
            .send(MatchmakingMessage::Create {
                outgoing_tx,
                session_tx,
                color_tx,
                join_code_tx,
            })
            .await?;
        let color = color_rx.await?;
        let join_code = join_code_rx.await?;
        Ok((session_rx, color, join_code))
    }

    // cancels a previously created game that has not yet been joined,
    pub async fn cancel_game(&self, join_code: JoinCode) -> anyhow::Result<()> {
        Ok(self
            .tx
            .send(MatchmakingMessage::Cancel { join_code })
            .await?)
    }

    // joins a previously created game
    pub async fn join_game(
        &self,
        join_code: JoinCode,
        outgoing_tx: mpsc::Sender<ServerMessage>,
    ) -> anyhow::Result<(SessionHandle, Color)> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(MatchmakingMessage::Join {
                response_tx,
                outgoing_tx,
                join_code,
            })
            .await?;
        response_rx
            .await?
            .ok_or(anyhow::anyhow!("failed to join game"))
    }
}
