use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::{
    keys::KeySpec,
    state::{Action, Pane, TuiState, Unit},
    view,
};

pub fn handle_key(state: &mut TuiState, key: KeyEvent) -> Action {
    if state.active_pane == Pane::Filter {
        return handle_filter_input(state, key);
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    for group in state.active_pane.keybinds() {
        for kb in *group {
            if matches!(kb.key, KeySpec::Ctrl(_)) && !ctrl {
                continue;
            }

            if key_matches(&kb.key, key.code) {
                return (kb.action)(state);
            }
        }
    }

    Action::None
}

fn key_matches(spec: &KeySpec, code: KeyCode) -> bool {
    use KeySpec as S;

    match (spec, code) {
        (S::Chars(chars), KeyCode::Char(c)) => chars.contains(c.to_ascii_lowercase()),
        (S::Ctrl(c), KeyCode::Char(ch)) => c == &ch.to_ascii_lowercase(),
        (S::Up, KeyCode::Up)
        | (S::Down, KeyCode::Down)
        | (S::Enter, KeyCode::Enter)
        | (S::Esc, KeyCode::Esc)
        | (S::Backspace, KeyCode::Backspace)
        | (S::Tab, KeyCode::Tab) => true,
        _ => false,
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
            state.active_pane = Pane::Main;
            state.filter_text.clear();
        }
        KeyCode::Enter => {
            state.active_pane = Pane::Main;
        }
        _ => return Action::None,
    }

    Action::Redraw
}

/// Move the cursor up (`is_up = true`) or down within the active pane.
/// Both panes clamp at the boundaries.
pub fn move_cursor(state: &mut TuiState, is_up: bool) -> Action {
    match state.active_pane {
        Pane::Main => {
            let view_len = state.view.len();
            if view_len == 0 {
                return Action::None;
            }

            let next = if is_up {
                state.selected.saturating_sub(1)
            } else {
                (state.selected + 1).min(view_len - 1)
            };

            let (selected, scroll_offset) =
                view::clamp_scroll(next, state.scroll_offset, state.visible_rows, view_len);

            state.selected = selected;
            state.scroll_offset = scroll_offset;
            state.selected_pid = state.view_pids.get(selected).copied();

            Action::Redraw
        }
        Pane::Unit => {
            let len = Unit::ALL.len();

            state.unit_picker_cursor = if is_up {
                state.unit_picker_cursor.saturating_sub(1)
            } else {
                (state.unit_picker_cursor + 1).min(len.saturating_sub(1))
            };

            Action::Redraw
        }
        _ => Action::None,
    }
}
