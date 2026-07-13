use crate::tui::{
    Action, input,
    state::{Pane, SortKey, TuiState, Unit},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeySpec {
    Chars(&'static str),
    Ctrl(char),
    Up,
    Down,
    Enter,
    Esc,
    Backspace,
    Tab,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Filtering,
    Sorting,
    Navigation,
    Other,
}

impl Section {
    pub const ALL: [Self; 4] = [
        Self::Sorting,
        Self::Navigation,
        Self::Filtering,
        Self::Other,
    ];

    pub const fn text(self) -> &'static str {
        match self {
            Self::Sorting => "Sorting",
            Self::Navigation => "Navigation",
            Self::Filtering => "Filtering",
            Self::Other => "Other",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HelpGroup {
    SortNums,
    Move,
}

impl HelpGroup {
    pub const fn text(self) -> &'static str {
        match self {
            Self::SortNums => "Sort by PID / Name / Sent / Recv / Total",
            Self::Move => "Move the cursor (it tracks the process)",
        }
    }
}

#[derive(Clone)]
pub struct Help {
    /// Whether this binding appears in the help popup.
    pub active: bool,

    /// Of which group is this keybind a part of in the help menu. Used to share
    /// a message for a group of keybinds.
    pub group: Option<HelpGroup>,

    /// Text to use in the help menu. Not used if the keybind is a part of a
    /// `HelpGroup`.
    pub text: &'static str,
}

#[derive(Clone)]
pub struct Keybind {
    /// Key sequence that activates the keybind.
    pub key: KeySpec,

    /// Short text for the keybind bar.
    pub label: &'static str,

    /// Of which section is this keybind a part of.
    pub section: Section,

    pub help: Help,

    /// Glyph shown in the help popup. If non-empty, overrides the
    /// `KeySpec`-derived glyph (used to share one representative glyph
    /// across a `HelpGroup`, e.g. "1-5" or "↑↓ jk"). Empty falls back to
    /// the `KeySpec`-derived glyph.
    pub help_glyph: &'static str,

    /// Whether this binding should appear in the keybind bar of its pane.
    pub bar: bool,

    pub action: fn(&mut TuiState) -> Action,
}

fn apply_sort(s: &mut TuiState, key: SortKey) -> Action {
    if s.sort_key == key {
        s.sort_dir = s.sort_dir.toggle();
    } else {
        s.sort_key = key;
        s.sort_dir = key.default_direction();
    }
    Action::Redraw
}

pub static MAIN_GROUP: [&[Keybind]; 4] = [&SORT_KEYS, &NAVIGATION_KEYS, &MAIN_KEYS, &QUIT_KEYS];
pub static UNIT_PICKER_GROUP: [&[Keybind]; 3] = [&UNIT_PICKER_KEYS, &NAVIGATION_KEYS, &QUIT_KEYS];
pub static HELP_GROUP: [&[Keybind]; 2] = [&HELP_KEYS, &QUIT_KEYS];
pub static FILTER_GROUP: [&[Keybind]; 1] = [&FILTER_KEYS];

/// Flat view of every keybind group, for the help popup. Each leaf array
/// appears exactly once.
pub static ALL: [&[Keybind]; 7] = [
    &MAIN_KEYS,
    &NAVIGATION_KEYS,
    &SORT_KEYS,
    &UNIT_PICKER_KEYS,
    &FILTER_KEYS,
    &HELP_KEYS,
    &QUIT_KEYS,
];

static QUIT_KEYS: [Keybind; 2] = [
    Keybind {
        key: KeySpec::Chars("q"),
        label: "quit",
        section: Section::Other,
        help: Help {
            active: true,
            group: None,
            text: "Quit (Ctrl-C also works)",
        },
        help_glyph: "",
        bar: true,
        action: |_| Action::Quit,
    },
    Keybind {
        key: KeySpec::Ctrl('c'),
        label: "",
        section: Section::Other,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: false,
        action: |_| Action::Quit,
    },
];

static MAIN_KEYS: [Keybind; 6] = [
    Keybind {
        key: KeySpec::Chars("p"),
        label: "pause",
        section: Section::Other,
        help: Help {
            active: true,
            group: None,
            text: "Pause / resume the live feed",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.paused = !s.paused;
            Action::Redraw
        },
    },
    Keybind {
        key: KeySpec::Chars("d"),
        label: "details",
        section: Section::Navigation,
        help: Help {
            active: true,
            group: None,
            text: "Toggle the per-process detail pane",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.show_detail = !s.show_detail;
            Action::Redraw
        },
    },
    Keybind {
        key: KeySpec::Chars("u"),
        label: "unit",
        section: Section::Other,
        help: Help {
            active: true,
            group: None,
            text: "Choose display unit (Auto/B/KB/MB/GB/TB)",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.active_pane = Pane::Unit;
            s.unit_picker_cursor = s.unit.index();
            Action::Redraw
        },
    },
    Keybind {
        key: KeySpec::Chars("/"),
        label: "filter",
        section: Section::Filtering,
        help: Help {
            active: true,
            group: None,
            text: "Start or edit filter",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.active_pane = Pane::Filter;
            Action::Redraw
        },
    },
    Keybind {
        key: KeySpec::Chars("?h"),
        label: "help",
        section: Section::Other,
        help: Help {
            active: true,
            group: None,
            text: "Toggle this help",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.active_pane = Pane::Help;
            Action::Redraw
        },
    },
    Keybind {
        key: KeySpec::Esc,
        label: "",
        section: Section::Filtering,
        help: Help {
            active: true,
            group: None,
            text: "Cancel input, or clear applied filter",
        },
        help_glyph: "",
        bar: false,
        action: |s| {
            if s.filter_text.is_empty() {
                Action::None
            } else {
                s.filter_text.clear();
                Action::Redraw
            }
        },
    },
];

static UNIT_PICKER_KEYS: [Keybind; 2] = [
    Keybind {
        key: KeySpec::Enter,
        label: "apply",
        section: Section::Other,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.unit = Unit::ALL[s.unit_picker_cursor];
            s.active_pane = Pane::Main;
            Action::Redraw
        },
    },
    Keybind {
        key: KeySpec::Esc,
        label: "cancel",
        section: Section::Other,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.active_pane = Pane::Main;
            Action::Redraw
        },
    },
];

static SORT_KEYS: [Keybind; 6] = [
    Keybind {
        key: KeySpec::Chars("r"),
        label: "reverse",
        section: Section::Sorting,
        help: Help {
            active: true,
            group: None,
            text: "Reverse current sort direction",
        },
        help_glyph: "",
        bar: true,
        action: |s| {
            s.sort_dir = s.sort_dir.toggle();
            Action::Redraw
        },
    },
    Keybind {
        key: KeySpec::Chars("1"),
        label: "pid",
        section: Section::Sorting,
        help: Help {
            active: true,
            group: Some(HelpGroup::SortNums),
            text: "",
        },
        help_glyph: "1-5",
        bar: true,
        action: |s| apply_sort(s, SortKey::Pid),
    },
    Keybind {
        key: KeySpec::Chars("2"),
        label: "name",
        section: Section::Sorting,
        help: Help {
            active: true,
            group: Some(HelpGroup::SortNums),
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |s| apply_sort(s, SortKey::Name),
    },
    Keybind {
        key: KeySpec::Chars("3"),
        label: "sent",
        section: Section::Sorting,
        help: Help {
            active: true,
            group: Some(HelpGroup::SortNums),
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |s| apply_sort(s, SortKey::Sent),
    },
    Keybind {
        key: KeySpec::Chars("4"),
        label: "recv",
        section: Section::Sorting,
        help: Help {
            active: true,
            group: Some(HelpGroup::SortNums),
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |s| apply_sort(s, SortKey::Recv),
    },
    Keybind {
        key: KeySpec::Chars("5"),
        label: "total",
        section: Section::Sorting,
        help: Help {
            active: true,
            group: Some(HelpGroup::SortNums),
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |s| apply_sort(s, SortKey::Total),
    },
];

// All four share `HelpGroup::Move`, only Up carries the help glyph.
static NAVIGATION_KEYS: [Keybind; 4] = [
    Keybind {
        key: KeySpec::Up,
        label: "move",
        section: Section::Navigation,
        help: Help {
            active: true,
            group: Some(HelpGroup::Move),
            text: "",
        },
        help_glyph: "↑↓ jk",
        bar: true,
        action: |s| input::move_cursor(s, true),
    },
    Keybind {
        key: KeySpec::Down,
        label: "move",
        section: Section::Navigation,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: false,
        action: |s| input::move_cursor(s, false),
    },
    Keybind {
        key: KeySpec::Chars("k"),
        label: "move",
        section: Section::Navigation,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: false,
        action: |s| input::move_cursor(s, true),
    },
    Keybind {
        key: KeySpec::Chars("j"),
        label: "move",
        section: Section::Navigation,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: false,
        action: |s| input::move_cursor(s, false),
    },
];

// `handle_filter_input` owns dispatch in the filter pane; these entries exist
// only to populate the keybind bar and the help popup. Their `action` is
// never invoked.
static FILTER_KEYS: [Keybind; 5] = [
    Keybind {
        key: KeySpec::Enter,
        label: "apply",
        section: Section::Filtering,
        help: Help {
            active: true,
            group: None,
            text: "Apply filter",
        },
        help_glyph: "",
        bar: true,
        action: |_| Action::None,
    },
    Keybind {
        key: KeySpec::Tab,
        label: "name ⇄ pid",
        section: Section::Filtering,
        help: Help {
            active: true,
            group: None,
            text: "Switch filter target (name ⇄ pid)",
        },
        help_glyph: "",
        bar: true,
        action: |_| Action::None,
    },
    Keybind {
        key: KeySpec::Esc,
        label: "cancel",
        section: Section::Filtering,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |_| Action::None,
    },
    Keybind {
        key: KeySpec::Backspace,
        label: "",
        section: Section::Filtering,
        help: Help {
            active: true,
            group: None,
            text: "Delete last character",
        },
        help_glyph: "",
        bar: false,
        action: |_| Action::None,
    },
    Keybind {
        key: KeySpec::Chars("?"),
        label: "help",
        section: Section::Other,
        help: Help {
            active: false,
            group: None,
            text: "",
        },
        help_glyph: "",
        bar: true,
        action: |_| Action::None,
    },
];

static HELP_KEYS: [Keybind; 1] = [Keybind {
    key: KeySpec::Esc,
    label: "hide",
    section: Section::Other,
    help: Help {
        active: false,
        group: None,
        text: "",
    },
    help_glyph: "",
    bar: true,
    action: |s| {
        s.active_pane = Pane::Main;
        Action::Redraw
    },
}];
