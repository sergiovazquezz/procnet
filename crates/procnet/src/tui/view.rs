use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

use procnet_core::stats::StatsRow;

use crate::tui::{
    state::{FilterTarget, Pane, SortDir, SortKey, TuiState, Unit},
    theme,
};

// TEST: Add a unit test, checking `rows`
pub fn sort_rows(rows: &mut [&StatsRow], key: SortKey, dir: SortDir) {
    rows.sort_by(|&a, &b| {
        let primary = match key {
            SortKey::Pid => a.pid.cmp(&b.pid),
            SortKey::Name => a.name.cmp(&b.name),
            SortKey::Sent => a.total().sent.cmp(&b.total().sent),
            SortKey::Recv => a.total().recv.cmp(&b.total().recv),
            SortKey::Total => a.total().combine().cmp(&b.total().combine()),
        };

        let primary = if dir == SortDir::Desc {
            primary.reverse()
        } else {
            primary
        };

        primary.then_with(|| a.pid.cmp(&b.pid))
    });
}

pub fn render(frame: &mut Frame, tick: u64, rows: &[StatsRow], state: &TuiState) {
    let mut constraints = vec![Constraint::Min(1), Constraint::Length(1)];
    if state.active_pane == Pane::Filter {
        constraints.push(Constraint::Length(1));
    }
    let layout = Layout::vertical(constraints).split(frame.area());

    render_table(frame, layout[0], tick, rows, state);
    render_keybind_bar(frame, layout[1], state);

    match state.active_pane {
        Pane::Filter => render_filter_prompt(frame, layout[2], state),
        Pane::Help => render_help(frame),
        Pane::Unit => render_unit_picker(frame, state),
        Pane::Command => {}
    }
}

fn render_table(frame: &mut Frame, area: Rect, tick: u64, rows: &[StatsRow], state: &TuiState) {
    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new("Waiting for process stats...")
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    }

    let mut view: Vec<&StatsRow> = Vec::with_capacity(rows.len());
    if state.filter_text.is_empty() {
        view.extend(rows.iter());
    } else {
        let needle = state.filter_text.to_ascii_lowercase();

        view.extend(rows.iter().filter(|&row| match state.filter_target {
            FilterTarget::Name => row.name.contains(&needle),
            FilterTarget::Pid => row.pid.to_string().contains(&needle),
        }));
    }

    sort_rows(&mut view, state.sort_key, state.sort_dir);

    let max_total = view.iter().map(|&r| r.total().combine()).max().unwrap_or(0);

    let table_rows = view.iter().map(|&row| {
        let total_style = if max_total > 0 {
            Style::new().fg(theme::traffic_color(
                row.total().combine() as f64 / max_total as f64,
            ))
        } else {
            Style::new()
        };

        Row::new([
            Cell::from(row.pid.to_string()),
            Cell::from(row.name.as_ref()),
            Cell::from(theme::format_bytes(row.total().sent, state.unit)),
            Cell::from(theme::format_bytes(row.total().recv, state.unit)),
            Cell::from(theme::format_bytes(row.total().combine(), state.unit)).style(total_style),
        ])
    });

    let header_cells = SortKey::ALL.map(|k| {
        let style = match k {
            SortKey::Sent => Style::new()
                .fg(theme::color::SENT)
                .add_modifier(Modifier::BOLD),
            SortKey::Recv => Style::new()
                .fg(theme::color::RECV)
                .add_modifier(Modifier::BOLD),
            _ => Style::new().fg(theme::color::MUTED),
        };
        Cell::from(k.label()).style(style)
    });

    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "procnet".to_string(),
            Style::new()
                .fg(theme::color::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        theme::muted_span("Sort:"),
        Span::raw(" "),
        theme::accent_span(
            format!("{} {}", state.sort_key.label(), state.sort_dir.arrow()).as_str(),
        ),
        Span::raw("  "),
        theme::muted_span("Filter:"),
        Span::raw(" "),
        filter_summary_span(state),
        Span::raw("  "),
        theme::muted_span("Unit:"),
        Span::raw(" "),
        theme::accent_span(state.unit.label()),
        Span::raw("  "),
        theme::muted_span("Tick:"),
        Span::raw(" "),
        theme::accent_span(tick.to_string().as_str()),
    ]);

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(8),
            Constraint::Min(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(Row::new(header_cells))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title_top(title.left_aligned()),
    )
    .column_spacing(1);

    frame.render_widget(table, area);
}

/// Build the right-hand side of the title: the effective filter expression.
fn filter_summary_span(state: &TuiState) -> Span<'static> {
    let text = state.filter_text.as_str();
    if text.is_empty() {
        return theme::muted_span("none");
    }
    Span::styled(
        format!("{}~\"{}\"", state.filter_target.label(), text),
        Style::new().fg(theme::color::ACCENT),
    )
}

fn render_keybind_bar(frame: &mut Frame, area: Rect, state: &TuiState) {
    let line = match state.active_pane {
        Pane::Filter => Line::from(vec![
            theme::key_span("Enter", false),
            theme::label_span("apply"),
            Span::raw(" "),
            theme::key_span("Tab", false),
            theme::label_span("name⇄pid"),
            Span::raw(" "),
            theme::key_span("Esc", false),
            theme::label_span("cancel"),
            theme::sep_span(),
            theme::key_span("?", false),
            theme::label_span("help"),
        ]),
        Pane::Unit => Line::from(vec![
            theme::key_span("↑↓", false),
            theme::label_span("move"),
            Span::raw(" "),
            theme::key_span("Enter", false),
            theme::label_span("apply"),
            Span::raw(" "),
            theme::key_span("Esc", false),
            theme::label_span("cancel"),
            theme::sep_span(),
            theme::key_span("?", false),
            theme::label_span("help"),
        ]),
        _ => {
            let mut spans: Vec<Span> = Vec::new();
            for (i, k) in SortKey::ALL.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                spans.push(theme::key_span(k.digit().to_string().as_str(), false));
                spans.push(theme::label_span(k.label()));
            }
            spans.push(theme::sep_span());
            spans.push(theme::key_span("r", false));
            spans.push(theme::label_span("reverse"));
            spans.push(theme::sep_span());
            spans.push(theme::key_span("u", false));
            spans.push(theme::label_span("unit"));
            spans.push(theme::sep_span());
            spans.push(theme::key_span("/", false));
            spans.push(theme::label_span("filter"));
            spans.push(theme::sep_span());
            spans.push(theme::key_span("?", false));
            spans.push(theme::label_span("help"));
            spans.push(theme::sep_span());
            spans.push(theme::key_span("q", false));
            spans.push(theme::label_span("quit"));
            Line::from(spans)
        }
    };

    frame.render_widget(Paragraph::new(line), area);
}

/// The table layout grows a third row only while this is visible.
fn render_filter_prompt(frame: &mut Frame, area: Rect, state: &TuiState) {
    let prompt = Line::from(vec![
        theme::muted_span(format!("Filter by {}: ", state.filter_target.label()).as_str()),
        Span::styled(
            state.filter_text.clone(),
            Style::new().fg(theme::color::ACCENT),
        ),
        Span::raw("_"),
    ]);
    frame.render_widget(Paragraph::new(prompt), area);
}

fn render_unit_picker(frame: &mut Frame, state: &TuiState) {
    let area = frame.area();
    let height = u16::try_from(Unit::ALL.len()).unwrap_or(6) + 4;
    let popup = centered_fixed(area, 22, height);

    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line> = Vec::with_capacity(Unit::ALL.len() + 1);
    lines.push(Line::from(vec![Span::styled(
        "unit".to_string(),
        Style::new()
            .fg(theme::color::ACCENT)
            .add_modifier(Modifier::BOLD),
    )]));

    for (i, u) in Unit::ALL.iter().enumerate() {
        let is_cursor = i == state.unit_picker_cursor;
        let is_applied = *u == state.unit;
        let prefix = if is_cursor { "▸ " } else { "  " };
        let label = u.label();
        let suffix = if is_applied { "  (current)" } else { "" };
        let style = if is_cursor {
            Style::new()
                .fg(theme::color::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new()
        };
        lines.push(Line::from(vec![Span::styled(
            format!("{prefix}{label}{suffix}"),
            style,
        )]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![theme::muted_span(
        "↑↓ move  Enter apply  Esc cancel",
    )]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme::color::ACCENT));

    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), popup);
}

fn render_help(frame: &mut Frame) {
    let area = frame.area();
    let popup = centered_fixed(area, 50, 17);

    frame.render_widget(Clear, popup);

    let lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(
                "procnet".to_string(),
                Style::new()
                    .fg(theme::color::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            theme::muted_span("— keybindings"),
        ]),
        Line::raw(""),
        section_header("Sorting"),
        help_line("1-5", "Sort by PID / Name / Sent / Recv / Total"),
        help_line("r", "Reverse current sort direction"),
        Line::raw(""),
        section_header("Filtering"),
        help_line("/", "Start or edit filter"),
        help_line("Tab", "Switch filter target (name ⇄ pid)"),
        help_line("Enter", "Apply filter"),
        help_line("BkSp", "Delete last character"),
        help_line("Esc", "Cancel input, or clear applied filter"),
        Line::raw(""),
        section_header("Other"),
        help_line("u", "Choose display unit (Auto/B/KB/MB/GB/TB)"),
        help_line("?  h", "Toggle this help"),
        help_line("q", "Quit  (Ctrl-C also works)"),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme::color::ACCENT));

    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), popup);
}

fn section_header(label: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        label.to_string(),
        Style::new()
            .fg(theme::color::ACCENT)
            .add_modifier(Modifier::BOLD),
    )])
}

fn help_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{key:<6}"),
            Style::new()
                .fg(theme::color::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        theme::muted_span(desc),
    ])
}

/// A centered rect with fixed width/height, clamped to `area`.
fn centered_fixed(area: Rect, width: u16, height: u16) -> Rect {
    let h = area.height.min(height);
    let w = area.width.min(width);

    let vertical = Layout::vertical([
        Constraint::Length(area.height.saturating_sub(h) / 2),
        Constraint::Length(h),
        Constraint::Min(0),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Length(area.width.saturating_sub(w) / 2),
        Constraint::Length(w),
        Constraint::Min(0),
    ])
    .split(vertical[1])[1]
}
