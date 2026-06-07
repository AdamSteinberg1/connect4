use futures::future::try_join;
use shared::{Board, Color, ColumnIndex, ServerMessage};
use tokio::sync::mpsc;

struct Session {
    rx: mpsc::Receiver<SessionMessage>,
    red_tx: mpsc::Sender<ServerMessage>,
    yellow_tx: mpsc::Sender<ServerMessage>,
    board: Board,
}

enum SessionMessage {
    MoveMessage { color: Color, column: ColumnIndex },
    PlayerDisconnected,
}

enum SessionOutcome {
    Ended,
    Ongoing,
}

#[derive(Clone)]
pub struct SessionHandle {
    tx: mpsc::Sender<SessionMessage>,
}

impl SessionHandle {
    pub fn new(
        red_tx: mpsc::Sender<ServerMessage>,
        yellow_tx: mpsc::Sender<ServerMessage>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let actor = Session::new(rx, red_tx, yellow_tx);
        tokio::spawn(actor.run());
        Self { tx }
    }

    pub async fn play_move(&self, color: Color, column: ColumnIndex) -> anyhow::Result<()> {
        Ok(self
            .tx
            .send(SessionMessage::MoveMessage { color, column })
            .await?)
    }

    pub async fn player_disconnected(&self) -> anyhow::Result<()> {
        Ok(self.tx.send(SessionMessage::PlayerDisconnected).await?)
    }

    pub async fn closed(&self) {
        self.tx.closed().await
    }
}

impl Session {
    fn new(
        rx: mpsc::Receiver<SessionMessage>,
        red_tx: mpsc::Sender<ServerMessage>,
        yellow_tx: mpsc::Sender<ServerMessage>,
    ) -> Self {
        Self {
            rx,
            red_tx,
            yellow_tx,
            board: Default::default(),
        }
    }

    async fn send_to_all(&mut self, msg: ServerMessage) -> anyhow::Result<()> {
        try_join(self.red_tx.send(msg.clone()), self.yellow_tx.send(msg)).await?;
        Ok(())
    }

    async fn run(self) {
        let result = self.try_run().await;
        if let Err(e) = result {
            eprintln!("session encountered an error: {:?}", e);
        }
    }

    async fn try_run(mut self) -> anyhow::Result<()> {
        try_join(
            self.red_tx.send(ServerMessage::GameStarted {
                your_color: Color::Red,
            }),
            self.yellow_tx.send(ServerMessage::GameStarted {
                your_color: Color::Yellow,
            }),
        )
        .await?;

        while let Some(message) = self.rx.recv().await {
            if let SessionOutcome::Ended = self.handle_message(message).await? {
                break;
            }
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: SessionMessage) -> anyhow::Result<SessionOutcome> {
        let (color, column) = match message {
            SessionMessage::MoveMessage { color, column } => (color, column),
            SessionMessage::PlayerDisconnected => {
                self.send_to_all(ServerMessage::OpponentDisconnected)
                    .await?;
                return Ok(SessionOutcome::Ended);
            }
        };

        match self.board.play_turn(column, color) {
            Ok(_) => {
                let response = ServerMessage::MovePlayed {
                    column,
                    color,
                    board: self.board.clone(),
                };
                let other_player_tx = match color {
                    Color::Yellow => &self.red_tx,
                    Color::Red => &self.yellow_tx,
                };
                other_player_tx.send(response).await?;
            }
            Err(e) => {
                let response = ServerMessage::InvalidMove(e);
                let response_tx = match color {
                    Color::Red => &self.red_tx,
                    Color::Yellow => &self.yellow_tx,
                };
                response_tx.send(response).await?;
            }
        }

        if let Some(winner) = self.board.get_winner() {
            self.send_to_all(ServerMessage::GameOver {
                winner: Some(winner),
            })
            .await?;
            return Ok(SessionOutcome::Ended);
        }

        if self.board.is_full() {
            self.send_to_all(ServerMessage::GameOver { winner: None })
                .await?;
            return Ok(SessionOutcome::Ended);
        }

        Ok(SessionOutcome::Ongoing)
    }
}
