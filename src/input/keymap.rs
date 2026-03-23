use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppMode {
    Normal,
    Command,
    Search,
    Form,
    Dialog,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    PageUp,
    PageDown,
    Select,
    Back,

    EnterCommandMode,
    EnterSearchMode,
    ToggleSidebar,

    Create,
    Delete,
    Edit,
    Refresh,

    NextField,
    PrevField,
    ToggleField,
    SubmitForm,
    CancelForm,

    Confirm,
    Deny,

    Quit,
    ForceQuit,

    CharInput(char),
    Unmapped,
}

pub struct KeyMap;

impl KeyMap {
    pub fn new() -> Self {
        Self
    }

    /// Resolve a key event to a semantic action based on current mode.
    pub fn resolve(&self, mode: AppMode, key: KeyEvent) -> KeyAction {
        // Force quit always works
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return KeyAction::ForceQuit;
        }

        match mode {
            AppMode::Normal => self.resolve_normal(key),
            AppMode::Command => self.resolve_command(key),
            AppMode::Search => self.resolve_search(key),
            AppMode::Form => self.resolve_form(key),
            AppMode::Dialog => self.resolve_dialog(key),
        }
    }

    /// Generate context help string for status bar.
    pub fn context_help(&self, mode: AppMode) -> String {
        match mode {
            AppMode::Normal => "j/k:move  Enter:select  /:search  ::cmd  Tab:sidebar  q:quit".into(),
            AppMode::Command => "Enter:run  Tab:complete  Up/Down:history  Esc:cancel".into(),
            AppMode::Search => "Enter:apply  Esc:cancel  (type to filter)".into(),
            AppMode::Form => "Tab:next  Shift+Tab:prev  Enter:submit  Esc:cancel".into(),
            AppMode::Dialog => "y:confirm  n/Esc:deny".into(),
        }
    }

    fn resolve_normal(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
            KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
            KeyCode::Char('g') => KeyAction::MoveToTop,
            KeyCode::Char('G') => KeyAction::MoveToBottom,
            KeyCode::PageUp => KeyAction::PageUp,
            KeyCode::PageDown => KeyAction::PageDown,
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                KeyAction::PageUp
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                KeyAction::PageDown
            }
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Esc => KeyAction::Back,
            KeyCode::Char(':') => KeyAction::EnterCommandMode,
            KeyCode::Char('/') => KeyAction::EnterSearchMode,
            KeyCode::Tab => KeyAction::ToggleSidebar,
            KeyCode::Char('c') => KeyAction::Create,
            KeyCode::Char('d') => KeyAction::Delete,
            KeyCode::Char('e') => KeyAction::Edit,
            KeyCode::Char('r') => KeyAction::Refresh,
            KeyCode::Char('q') => KeyAction::Quit,
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_command(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Esc => KeyAction::Back,
            KeyCode::Tab => KeyAction::NextField,
            KeyCode::Up => KeyAction::MoveUp,
            KeyCode::Down => KeyAction::MoveDown,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_search(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Esc => KeyAction::Back,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_form(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Tab => KeyAction::NextField,
            KeyCode::BackTab => KeyAction::PrevField,
            KeyCode::Enter => KeyAction::SubmitForm,
            KeyCode::Esc => KeyAction::CancelForm,
            KeyCode::Char(' ') => KeyAction::ToggleField,
            KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
            KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_dialog(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => KeyAction::Confirm,
            KeyCode::Char('n') | KeyCode::Char('N') => KeyAction::Deny,
            KeyCode::Esc => KeyAction::Deny,
            KeyCode::Enter => KeyAction::Confirm,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_normal_mode_navigation() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('j'))), KeyAction::MoveDown);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('k'))), KeyAction::MoveUp);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('g'))), KeyAction::MoveToTop);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('G'))), KeyAction::MoveToBottom);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Enter)), KeyAction::Select);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Esc)), KeyAction::Back);
    }

    #[test]
    fn test_normal_mode_switching() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char(':'))), KeyAction::EnterCommandMode);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('/'))), KeyAction::EnterSearchMode);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Tab)), KeyAction::ToggleSidebar);
    }

    #[test]
    fn test_normal_mode_actions() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('c'))), KeyAction::Create);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('r'))), KeyAction::Refresh);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('q'))), KeyAction::Quit);
    }

    #[test]
    fn test_command_mode() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Enter)), KeyAction::Select);
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Esc)), KeyAction::Back);
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Tab)), KeyAction::NextField);
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Char('a'))), KeyAction::CharInput('a'));
    }

    #[test]
    fn test_search_mode() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Search, key(KeyCode::Enter)), KeyAction::Select);
        assert_eq!(km.resolve(AppMode::Search, key(KeyCode::Esc)), KeyAction::Back);
        assert_eq!(km.resolve(AppMode::Search, key(KeyCode::Char('w'))), KeyAction::CharInput('w'));
    }

    #[test]
    fn test_force_quit_any_mode() {
        let km = KeyMap::new();
        let ctrl_c = key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(AppMode::Normal, ctrl_c), KeyAction::ForceQuit);
        assert_eq!(km.resolve(AppMode::Command, ctrl_c), KeyAction::ForceQuit);
        assert_eq!(km.resolve(AppMode::Form, ctrl_c), KeyAction::ForceQuit);
    }

    #[test]
    fn test_context_help() {
        let km = KeyMap::new();
        let help = km.context_help(AppMode::Normal);
        assert!(help.contains("j/k"));
        assert!(help.contains("quit"));

        let help = km.context_help(AppMode::Command);
        assert!(help.contains("Tab"));
        assert!(help.contains("history"));
    }
}
