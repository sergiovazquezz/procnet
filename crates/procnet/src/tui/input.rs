use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::state::{Action, Pane, SortKey, TuiState, Unit};

pub fn handle_key(state: &mut TuiState, key: KeyEvent) -> Action {
    match state.active_pane {
        Pane::Help => handle_help_modal(state, key),
        Pane::Unit => handle_unit_picker(state, key),
        Pane::Filter => handle_filter_input(state, key),
        Pane::Command => handle_command(state, key),
    }
}

#[expect(clippy::missing_const_for_fn)]
fn handle_help_modal(state: &mut TuiState, key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char('?' | 'h' | 'H') | KeyCode::Esc => {
            state.active_pane = Pane::Command;
            Action::Redraw
        }
        KeyCode::Char('q' | 'Q') => Action::Quit,
        KeyCode::Char('c' | 'C') if ctrl => Action::Quit,
        _ => Action::None,
    }
}

#[expect(clippy::missing_const_for_fn)]
fn handle_unit_picker(state: &mut TuiState, key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let len = Unit::ALL.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k' | 'K') => {
            state.unit_picker_cursor = (state.unit_picker_cursor + len - 1) % len;
            Action::Redraw
        }
        KeyCode::Down | KeyCode::Char('j' | 'J') => {
            state.unit_picker_cursor = (state.unit_picker_cursor + 1) % len;
            Action::Redraw
        }
        KeyCode::Enter => {
            state.unit = Unit::ALL[state.unit_picker_cursor];
            state.active_pane = Pane::Command;
            Action::Redraw
        }
        KeyCode::Esc | KeyCode::Char('u' | 'U') => {
            state.active_pane = Pane::Command;
            Action::Redraw
        }
        KeyCode::Char('q' | 'Q') => Action::Quit,
        KeyCode::Char('c' | 'C') if ctrl => Action::Quit,
        _ => Action::None,
    }
}

fn handle_filter_input(state: &mut TuiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char(c) => {
            state.filter_text.push(c);
        }
        KeyCode::Backspace => {
            state.filter_text.pop();
        }
        KeyCode::Tab => {
            state.filter_target = state.filter_target.toggle();
        }
        KeyCode::Esc => {
            state.active_pane = Pane::Command;
            state.filter_text.clear();
        }
        KeyCode::Enter => {
            state.active_pane = Pane::Command;
        }
        _ => return Action::None,
    }

    Action::Redraw
}

fn handle_command(state: &mut TuiState, key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char('q' | 'Q') => Action::Quit,
        KeyCode::Char('c' | 'C') if ctrl => Action::Quit,
        KeyCode::Char('r' | 'R') => {
            state.sort_dir = state.sort_dir.toggle();
            Action::Redraw
        }
        KeyCode::Char('u' | 'U') => {
            state.active_pane = Pane::Unit;
            state.unit_picker_cursor = state.unit.index();
            Action::Redraw
        }
        KeyCode::Char(d) => {
            if d == '?' || d == 'h' || d == 'H' {
                state.active_pane = Pane::Help;
                Action::Redraw
            } else if let Some(new_key) = SortKey::from_digit(d) {
                if state.sort_key == new_key {
                    state.sort_dir = state.sort_dir.toggle();
                } else {
                    state.sort_key = new_key;
                    state.sort_dir = new_key.default_direction();
                }
                Action::Redraw
            } else if d == '/' {
                state.active_pane = Pane::Filter;
                Action::Redraw
            } else {
                Action::None
            }
        }
        KeyCode::Esc => {
            if state.filter_text.is_empty() {
                Action::None
            } else {
                state.filter_text.clear();
                Action::Redraw
            }
        }
        _ => Action::None,
    }
}
