use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::{
    state::{Action, Pane, SortKey, TuiState, Unit},
    view::clamp_scroll,
};

#[derive(Clone, Copy)]
enum Direction {
    Down,
    Up,
}

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
        KeyCode::Up | KeyCode::Char('k' | 'K') => {
            move_cursor(state, Direction::Up);
            Action::Redraw
        }
        KeyCode::Down | KeyCode::Char('j' | 'J') => {
            move_cursor(state, Direction::Down);
            Action::Redraw
        }
        KeyCode::Char('p' | 'P') => {
            state.paused = !state.paused;
            Action::Redraw
        }
        KeyCode::Char('d' | 'D') => {
            state.show_detail = !state.show_detail;
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

/// Move the cursor within the last rendered view, keeping it on screen. The
/// cursor locks onto the PID at the new index so it tracks that process across
/// ticks.
fn move_cursor(state: &mut TuiState, direction: Direction) {
    if state.view_len == 0 {
        return;
    }

    let next = match direction {
        Direction::Down => (state.selected + 1).min(state.view_len - 1),
        Direction::Up => state.selected.saturating_sub(1),
    };

    let (selected, scroll_offset) = clamp_scroll(
        next,
        state.scroll_offset,
        state.visible_rows,
        state.view_len,
    );

    state.selected = selected;
    state.scroll_offset = scroll_offset;
    state.selected_pid = state.view_pids.get(selected).copied();
}
