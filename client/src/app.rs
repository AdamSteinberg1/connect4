use crate::game::Game;
use crate::input::key_to_input;
use crate::menu::{Menu, MenuItem};
use crate::net::Connection;
use crate::online::Online;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{DefaultTerminal, Frame};
use shared::{ClientMessage, ServerMessage};
use std::io;
use tokio::select;

enum Screen {
    Menu(Menu),
    Local(Game),
    Online(Online),
}

pub struct App {
    screen: Screen,
    /// opened the first time an online option is chosen, then reused for subsequent games.
    /// dropped when a game in progress is abandoned or the connection is lost
    connection: Option<Connection>,
    server_addr: String,
    should_quit: bool,
}

impl App {
    pub fn new(server_addr: String) -> Self {
        Self {
            screen: Screen::Menu(Menu::default()),
            connection: None,
            server_addr,
            should_quit: false,
        }
    }

    /// runs the application's main loop until the user quits
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut events = EventStream::new();
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            select! {
                event = events.next() => match event {
                    Some(event) => self.handle_event(event?).await,
                    None => self.should_quit = true,
                },
                message = next_server_message(&mut self.connection) => {
                    self.handle_server_message(message);
                }
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        match &self.screen {
            Screen::Menu(menu) => frame.render_widget(menu, area),
            Screen::Local(game) => frame.render_widget(game, area),
            Screen::Online(online) => frame.render_widget(online, area),
        }
    }

    async fn handle_event(&mut self, event: Event) {
        let Event::Key(key_event) = event else {
            return;
        };
        if key_event.kind != KeyEventKind::Press {
            return;
        }
        // raw mode swallows the usual ctrl-c signal, so handle it here
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.should_quit = true;
            return;
        }

        let code = key_event.code;
        match self.screen {
            Screen::Menu(_) => self.handle_menu_key(code).await,
            Screen::Local(_) => self.handle_local_key(code),
            Screen::Online(_) => self.handle_online_key(code),
        }
    }

    async fn handle_menu_key(&mut self, code: KeyCode) {
        let Screen::Menu(menu) = &mut self.screen else {
            return;
        };
        match code {
            KeyCode::Up => menu.move_up(),
            KeyCode::Down => menu.move_down(),
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Enter => {
                let item = menu.selected_item();
                self.start(item).await;
            }
            _ => {}
        }
    }

    async fn start(&mut self, item: MenuItem) {
        match item {
            MenuItem::LocalGame => self.screen = Screen::Local(Game::new()),
            MenuItem::CreateOnlineGame => {
                if self.ensure_connected().await {
                    self.send(ClientMessage::CreateGame);
                    self.screen = Screen::Online(Online::Creating);
                }
            }
            MenuItem::JoinOnlineGame => {
                if self.ensure_connected().await {
                    self.screen = Screen::Online(Online::EnteringCode {
                        input: String::new(),
                        error: None,
                    });
                }
            }
        }
    }

    /// opens the connection if there isn't one. On failure, returns to the menu with an error
    async fn ensure_connected(&mut self) -> bool {
        if self.connection.is_some() {
            return true;
        }
        match Connection::open(&self.server_addr).await {
            Ok(connection) => {
                self.connection = Some(connection);
                true
            }
            Err(error) => {
                self.screen = Screen::Menu(Menu::with_error(format!(
                    "Could not connect to {}: {error}",
                    self.server_addr
                )));
                false
            }
        }
    }

    fn handle_local_key(&mut self, code: KeyCode) {
        let Screen::Local(game) = &mut self.screen else {
            return;
        };
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => self.screen = Screen::Menu(Menu::default()),
            code => {
                if let Some(input) = key_to_input(code) {
                    game.handle_input(input);
                }
            }
        }
    }

    fn handle_online_key(&mut self, code: KeyCode) {
        let Screen::Online(online) = &self.screen else {
            return;
        };
        // 'q' is a valid join code character
        let entering_code = matches!(online, Online::EnteringCode { .. });
        let is_over = online.is_over();

        match code {
            KeyCode::Char('q') if !entering_code => self.should_quit = true,
            KeyCode::Esc => self.leave_online(),
            KeyCode::Enter if is_over => self.leave_online(),
            code => {
                let mut message = None;
                self.update_online(|online| {
                    let (online, outgoing) = online.handle_key(code);
                    message = outgoing;
                    online
                });
                if let Some(message) = message {
                    self.send(message);
                }
            }
        }
    }

    /// returns to the menu. Abandoning a live session drops the connection,
    /// so the server cancels the game or tells the opponent we left
    fn leave_online(&mut self) {
        let in_session = matches!(&self.screen, Screen::Online(online) if online.is_in_session());
        if in_session {
            self.connection = None;
        }
        self.screen = Screen::Menu(Menu::default());
    }

    /// `None` means the connection closed
    fn handle_server_message(&mut self, message: Option<ServerMessage>) {
        match message {
            Some(message) => self.update_online(|online| online.on_message(message)),
            None => {
                self.connection = None;
                self.update_online(Online::on_connection_lost);
            }
        }
    }

    /// applies a state transition if we are on the online screen, otherwise does nothing
    fn update_online(&mut self, transition: impl FnOnce(Online) -> Online) {
        let screen = std::mem::replace(&mut self.screen, Screen::Menu(Menu::default()));
        self.screen = match screen {
            Screen::Online(online) => Screen::Online(transition(online)),
            other => other,
        };
    }

    fn send(&self, message: ClientMessage) {
        if let Some(connection) = &self.connection {
            connection.send(message);
        }
    }
}

/// resolves with the next server message, or None if the connection closed.
/// stays pending forever while there is no connection
async fn next_server_message(connection: &mut Option<Connection>) -> Option<ServerMessage> {
    match connection {
        Some(connection) => connection.recv().await,
        None => std::future::pending().await,
    }
}
