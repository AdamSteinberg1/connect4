#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    LocalGame,
    CreateOnlineGame,
    JoinOnlineGame,
}

impl MenuItem {
    pub const ALL: [MenuItem; 3] = [
        MenuItem::LocalGame,
        MenuItem::CreateOnlineGame,
        MenuItem::JoinOnlineGame,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MenuItem::LocalGame => "Local game",
            MenuItem::CreateOnlineGame => "Create online game",
            MenuItem::JoinOnlineGame => "Join online game",
        }
    }
}

#[derive(Default)]
pub struct Menu {
    selected: usize,
    error_message: Option<String>,
}

impl Menu {
    pub fn with_error(error_message: String) -> Self {
        Self {
            selected: 0,
            error_message: Some(error_message),
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(MenuItem::ALL.len() - 1);
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected_item(&self) -> MenuItem {
        MenuItem::ALL[self.selected]
    }

    pub fn error(&self) -> Option<&str> {
        self.error_message.as_deref()
    }
}
