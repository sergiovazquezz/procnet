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

    pub fn label(self) -> &'static str {
        match self {
            Self::Pid => "PID",
            Self::Name => "Name",
            Self::Sent => "Sent",
            Self::Recv => "Received",
            Self::Total => "Total",
        }
    }

    pub fn from_digit(d: char) -> Option<Self> {
        match d {
            '1' => Some(Self::Pid),
            '2' => Some(Self::Name),
            '3' => Some(Self::Sent),
            '4' => Some(Self::Recv),
            '5' => Some(Self::Total),
            _ => None,
        }
    }

    /// Inverse of `from_digit`: the digit key that selects this column.
    pub fn digit(self) -> char {
        match self {
            Self::Pid => '1',
            Self::Name => '2',
            Self::Sent => '3',
            Self::Recv => '4',
            Self::Total => '5',
        }
    }

    pub fn default_direction(self) -> SortDir {
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
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Asc => "▲",
            Self::Desc => "▼",
        }
    }

    pub fn toggle(self) -> Self {
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Pid => "pid",
        }
    }

    pub fn toggle(self) -> Self {
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

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::B => "B",
            Self::Kb => "KB",
            Self::Mb => "MB",
            Self::Gb => "GB",
            Self::Tb => "TB",
        }
    }

    /// Divisor used when formatting with a fixed unit. `Auto` returns 0
    /// (unused — auto formatting picks a divisor per value).
    pub fn divisor(self) -> u64 {
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Help,
    Unit,
    Filter,
    Command,
}

pub struct TuiState {
    pub sort_key: SortKey,
    pub sort_dir: SortDir,
    pub active_pane: Pane,
    pub filter_target: FilterTarget,
    /// Committed filter; empty string means no filtering.
    pub filter_text: String,
    /// Display unit for byte counts.
    pub unit: Unit,
    /// Cursor row inside the unit picker (index into `Unit::ALL`).
    pub unit_picker_cursor: usize,
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiState {
    pub fn new() -> Self {
        Self {
            sort_key: SortKey::Total,
            sort_dir: SortDir::Desc,
            active_pane: Pane::Command,
            filter_target: FilterTarget::Name,
            filter_text: String::new(),
            unit: Unit::Auto,
            unit_picker_cursor: Unit::Auto.index(),
        }
    }
}
