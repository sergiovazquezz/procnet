use procnet_core::stats::MAP_SIZE;

use crate::tui::keys;
use crate::tui::keys::Keybind;

pub struct TuiState {
    pub sort_key: SortKey,

    pub sort_dir: SortDir,

    pub active_pane: Pane,

    /// Which table the main pane shows.
    pub view_mode: ViewMode,

    pub filter_target: FilterTarget,

    /// Committed filter. Empty string means no filtering.
    pub filter_text: String,

    /// Display unit for data.
    pub unit: Unit,

    /// Cursor row inside the unit picker (index into `Unit::ALL`).
    pub unit_picker_cursor: usize,

    /// PID of the row the cursor is locked onto. `None` means the cursor
    /// floats on the top row. It does not track a specific process until the
    /// user moves it.
    pub selected_pid: Option<u32>,

    /// Resolved index of the cursor in the last rendered view. Written by
    /// `render_table`, read by the input handler to move up/down.
    pub selected: usize,

    /// First visible row index into the filtered+sorted view.
    pub scroll_offset: usize,

    /// Indices into the most recent `snap.rows` forming the filtered+sorted
    /// view. Reused across renders via clear+extend to avoid per-frame
    /// allocation.
    pub view: Vec<usize>,

    /// PIDs of the current filtered+sorted view, refreshed each render so the
    /// input handler can move the cursor without access to the snapshot.
    pub view_pids: Vec<u32>,

    /// How many table rows fit in the area at the last render.
    pub visible_rows: u16,

    /// Whether the live snapshot feed is frozen.
    pub paused: bool,

    /// Whether the per-process detail pane is shown.
    pub show_detail: bool,
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiState {
    pub fn new() -> Self {
        Self {
            sort_key: SortKey::Name,
            sort_dir: SortDir::Desc,
            active_pane: Pane::Main,
            view_mode: ViewMode::Live,
            filter_target: FilterTarget::Name,
            filter_text: String::new(),
            unit: Unit::Auto,
            unit_picker_cursor: Unit::Auto.index(),
            selected_pid: None,
            selected: 0,
            scroll_offset: 0,
            view: Vec::with_capacity(MAP_SIZE),
            view_pids: Vec::new(),
            visible_rows: 0,
            paused: false,
            show_detail: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Pid,
    Name,
    Sent,
    Recv,
    Total,
}

impl SortKey {
    // TEST: Ensure that all of the variants are a part of `ALL`
    pub const ALL: [Self; 5] = [Self::Pid, Self::Name, Self::Sent, Self::Recv, Self::Total];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pid => "pid",
            Self::Name => "name",
            Self::Sent => "sent",
            Self::Recv => "received",
            Self::Total => "total",
        }
    }

    pub const fn default_direction(self) -> SortDir {
        match self {
            Self::Pid | Self::Name => SortDir::Asc,
            Self::Sent | Self::Recv | Self::Total => SortDir::Desc,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub const fn arrow(self) -> &'static str {
        match self {
            Self::Asc => "▲",
            Self::Desc => "▼",
        }
    }

    pub const fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FilterTarget {
    Name,
    Pid,
}

impl FilterTarget {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Pid => "pid",
        }
    }

    pub const fn toggle(self) -> Self {
        match self {
            Self::Name => Self::Pid,
            Self::Pid => Self::Name,
        }
    }
}

/// Display unit for byte counts. `Auto` picks a unit per value (mixed units
/// across rows); the fixed variants lock every row to the same unit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Auto,
    B,
    Kb,
    Mb,
    Gb,
    Tb,
}

impl Unit {
    // TEST: Ensure that all of the variants are a part of `ALL`
    pub const ALL: [Self; 6] = [Self::Auto, Self::B, Self::Kb, Self::Mb, Self::Gb, Self::Tb];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::B => "B",
            Self::Kb => "KB",
            Self::Mb => "MB",
            Self::Gb => "GB",
            Self::Tb => "TB",
        }
    }

    /// Divisor used when formatting with a fixed unit. `Auto` returns 0
    /// (unused — auto formatting picks a divisor per value).
    pub const fn divisor(self) -> u64 {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * 1024 * 1024;
        const TB: u64 = 1024 * 1024 * 1024 * 1024;

        match self {
            Self::Auto => 0,
            Self::B => 1,
            Self::Kb => KB,
            Self::Mb => MB,
            Self::Gb => GB,
            Self::Tb => TB,
        }
    }

    /// Position in `ALL`, used to initialize the picker cursor on open.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|u| *u == self).unwrap_or(0)
    }
}

/// Outcome of a single input event, consumed by the app loop.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Redraw,
    None,
}

/// Which table the main pane is currently showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Live,
    Dead,
}

impl ViewMode {
    pub const fn toggle(self) -> Self {
        match self {
            Self::Live => Self::Dead,
            Self::Dead => Self::Live,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Main,
    Help,
    Unit,
    Filter,
}

impl Pane {
    pub fn keybinds(self) -> &'static [&'static [Keybind]] {
        match self {
            Self::Main => &keys::MAIN_GROUP,
            Self::Unit => &keys::UNIT_PICKER_GROUP,
            Self::Help => &keys::HELP_GROUP,
            Self::Filter => &keys::FILTER_GROUP,
        }
    }
}
