use futures::future::try_join;
use shared::{Board, Color, ColumnIndex, ServerMessage};
use tokio::sync::mpsc;

pub struct Session {
    rx: mpsc::Receiver<SessionMessage>, // we receive moves from both players on one channel
    red_tx: mpsc::Sender<ServerMessage>,
    yellow_tx: mpsc::Sender<ServerMessage>,
    board: Board,
}

//message sent to a session
pub enum SessionMessage {
    MoveMessage { color: Color, column: ColumnIndex },
    PlayerDisconnected,
}

enum SessionOutcome {
    Ended,
    Ongoing,
}

impl Session {
    pub fn new(
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

    async fn send_to_all_players(&mut self, msg: ServerMessage) -> anyhow::Result<()> {
        try_join(self.red_tx.send(msg.clone()), self.yellow_tx.send(msg)).await?;
        Ok(())
    }

    pub(crate) async fn run(mut self) -> anyhow::Result<()> {
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
                self.send_to_all_players(ServerMessage::OpponentDisconnected)
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
            let msg = ServerMessage::GameOver {
                winner: Some(winner),
            };
            self.send_to_all_players(msg).await?;
            return Ok(SessionOutcome::Ended);
        }
        if self.board.is_full() {
            let msg = ServerMessage::GameOver { winner: None };
            self.send_to_all_players(msg).await?;
            return Ok(SessionOutcome::Ended);
        }

        Ok(SessionOutcome::Ongoing)
    }
}
