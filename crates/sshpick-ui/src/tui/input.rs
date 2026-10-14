use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use sshpick_core::Key;

pub(super) fn key(event: KeyEvent) -> Option<Key> {
    if event.kind != KeyEventKind::Press {
        return None;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = event.modifiers.contains(KeyModifiers::ALT);
    match event.code {
        KeyCode::Char(c) if ctrl => Some(Key::Ctrl(c.to_ascii_lowercase())),
        KeyCode::Char(c) if !alt => Some(Key::Char(c)),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_maps_without_policy() {
        let press = |code, modifiers| key(KeyEvent::new(code, modifiers));
        let ctrl = KeyModifiers::CONTROL;
        assert_eq!(press(KeyCode::Char('p'), ctrl), Some(Key::Ctrl('p')));
        assert_eq!(press(KeyCode::Char('J'), ctrl), Some(Key::Ctrl('j')));
        assert_eq!(
            press(KeyCode::Char('A'), KeyModifiers::SHIFT),
            Some(Key::Char('A'))
        );
        assert_eq!(press(KeyCode::Char('x'), KeyModifiers::ALT), None);
        assert_eq!(press(KeyCode::Left, KeyModifiers::NONE), None);
    }
}
