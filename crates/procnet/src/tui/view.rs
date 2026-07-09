use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

use procnet_core::{
    ipc::SnapshotData,
    stats::{StatsBytes, StatsRow},
};

use crate::tui::{
    state::{FilterTarget, Pane, SortDir, SortKey, TuiState, Unit},
    theme,
};

pub fn sort_rows(rows: &[StatsRow], indices: &mut [usize], key: SortKey, dir: SortDir) {
    indices.sort_by(|&a, &b| {
        let (ra, rb) = (&rows[a], &rows[b]);
        let primary = match key {
            SortKey::Pid => ra.pid.cmp(&rb.pid),
            SortKey::Name => ra.name.cmp(&rb.name),
            SortKey::Sent => ra.total().sent.cmp(&rb.total().sent),
            SortKey::Recv => ra.total().recv.cmp(&rb.total().recv),
            SortKey::Total => ra.total().combine().cmp(&rb.total().combine()),
        };

        let primary = if dir == SortDir::Desc {
            primary.reverse()
        } else {
            primary
        };

        primary.then_with(|| ra.pid.cmp(&rb.pid))
    });
}

/// Clamp the cursor and scroll offset so the selected row stays inside the
/// visible window. `num_visible_rows` is how many rows fit. `len` is the view
/// length. The offset is also prevented from scrolling past the last page (no
/// empty space at the bottom once everything fits).
pub fn clamp_scroll(
    selected: usize,
    offset: usize,
    num_visible_rows: u16,
    len: usize,
) -> (usize, usize) {
    if len == 0 {
        return (0, 0);
    }

    let selected = selected.min(len - 1);
    let vis = num_visible_rows as usize;

    if vis == 0 || len <= vis {
        return (selected, 0);
    }

    let mut offset = offset.min(len - vis);

    if selected < offset {
        offset = selected;
    } else if selected > offset + vis - 1 {
        offset = selected + 1 - vis;
    }

    (selected, offset)
}

pub fn render(frame: &mut Frame, snap: &SnapshotData, state: &mut TuiState) {
    state.view.clear();
    if !snap.rows.is_empty() {
        if state.filter_text.is_empty() {
            state.view.extend(0..snap.rows.len());
        } else {
            let needle = state.filter_text.to_ascii_lowercase();
            let filt = state.filter_target;
            state.view.extend(
                snap.rows
                    .iter()
                    .enumerate()
                    .filter_map(|(i, row)| match filt {
                        FilterTarget::Name => row.name.contains(&needle).then_some(i),
                        FilterTarget::Pid => row.pid.to_string().contains(&needle).then_some(i),
                    }),
            );
        }

        sort_rows(&snap.rows, &mut state.view, state.sort_key, state.sort_dir);
    }

    let mut constraints = vec![Constraint::Min(1)];
    if state.show_detail {
        constraints.push(Constraint::Length(8));
    }
    constraints.push(Constraint::Length(1));
    if state.active_pane == Pane::Filter {
        constraints.push(Constraint::Length(1));
    }
    let layout = Layout::vertical(constraints).split(frame.area());

    let mut idx = 0;
    render_table(frame, layout[idx], snap, state);
    idx += 1;
    if state.show_detail {
        render_detail(frame, layout[idx], snap, state);
        idx += 1;
    }
    render_keybind_bar(frame, layout[idx], state);
    idx += 1;

    match state.active_pane {
        Pane::Filter => render_filter_prompt(frame, layout[idx], state),
        Pane::Help => render_help(frame),
        Pane::Unit => render_unit_picker(frame, state),
        Pane::Command => {}
    }
}

fn render_table(frame: &mut Frame, area: Rect, snap: &SnapshotData, state: &mut TuiState) {
    if snap.rows.is_empty() {
        state.visible_rows = 0;
        state.view_pids.clear();
        frame.render_widget(
            Paragraph::new("Waiting for process stats...")
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    }

    if state.view.is_empty() {
        state.visible_rows = 0;
        state.view_pids.clear();

        frame.render_widget(
            Paragraph::new("No processes match the filter...")
                .block(Block::default().borders(Borders::ALL)),
            area,
        );

        return;
    }

    // Resolve the cursor: if locked onto a PID, follow it through the view;
    // otherwise float on the top row. A PID that has vanished releases the
    // lock so the cursor returns to the top instead of snapping to a stranger.
    let (selected, resolved_pid) = state
        .selected_pid
        .and_then(|pid| {
            state
                .view
                .iter()
                .position(|&i| snap.rows[i].pid == pid)
                .map(|i| (i, Some(pid)))
        })
        .unwrap_or((0, None));

    let visible_rows = area.height.saturating_sub(3);
    let (selected, scroll_offset) = clamp_scroll(
        selected,
        state.scroll_offset,
        visible_rows,
        state.view.len(),
    );

    let max_total = state
        .view
        .iter()
        .map(|&i| snap.rows[i].total().combine())
        .max()
        .unwrap_or(0);

    state.selected = selected;
    state.scroll_offset = scroll_offset;
    state.visible_rows = visible_rows;
    state.view_pids.clear();
    state
        .view_pids
        .extend(state.view.iter().map(|&i| snap.rows[i].pid));
    state.selected_pid = resolved_pid;

    render_table_widget(
        frame,
        area,
        snap,
        state,
        ViewWindow {
            selected,
            scroll_offset,
            visible_rows,
            max_total,
        },
    );
}

struct ViewWindow {
    selected: usize,
    scroll_offset: usize,
    visible_rows: u16,
    max_total: u64,
}

fn render_table_widget(
    frame: &mut Frame,
    area: Rect,
    snap: &SnapshotData,
    state: &TuiState,
    window: ViewWindow,
) {
    let view_len = state.view.len();
    let end = (window.scroll_offset + window.visible_rows as usize).min(view_len);
    let display = &state.view[window.scroll_offset..end];

    let table_rows = display.iter().enumerate().map(|(i, &idx)| {
        let row = &snap.rows[idx];
        let is_selected = window.scroll_offset + i == window.selected;
        let base_style = if is_selected {
            Style::new()
                .bg(theme::color::ACCENT)
                .fg(theme::color::INVERT_TEXT)
        } else {
            Style::new()
        };

        let total_style = if is_selected {
            base_style
        } else if window.max_total > 0 {
            Style::new().fg(theme::traffic_color(
                row.total().combine() as f64 / window.max_total as f64,
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
        .style(base_style)
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

    let title = table_title(state, snap.tick, snap.interval);

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
    .block(Block::default().borders(Borders::ALL).title_top(title))
    .column_spacing(1);

    frame.render_widget(table, area);
}

fn render_detail(frame: &mut Frame, area: Rect, snap: &SnapshotData, state: &TuiState) {
    let Some(&idx) = state.view.get(state.selected) else {
        frame.render_widget(
            Paragraph::new("No process selected").block(detail_block("details")),
            area,
        );
        return;
    };
    let row = &snap.rows[idx];

    let unit = state.unit;
    let (tcp_cum, udp_cum) = row.cum();
    let total = row.total();
    let total_cum = row.total_cum();

    let lines = vec![
        Line::from(vec![
            Span::styled(
                row.pid.to_string(),
                Style::new()
                    .fg(theme::color::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                row.name.as_ref().to_string(),
                Style::new().fg(theme::color::ACCENT),
            ),
        ]),
        proto_line("TCP", row.tcp, tcp_cum, unit),
        proto_line("UDP", row.udp, udp_cum, unit),
        Line::raw(""),
        proto_line("Total", total, total_cum, unit),
    ];

    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(detail_block("details")),
        area,
    );
}

fn proto_line(label: &str, tick: StatsBytes, cum: StatsBytes, unit: Unit) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<5}"),
            Style::new()
                .fg(theme::color::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        theme::muted_span("sent: "),
        Span::styled(
            format!(
                "{} / {}",
                theme::format_bytes(tick.sent, unit),
                theme::format_bytes(cum.sent, unit)
            ),
            Style::new().fg(theme::color::SENT),
        ),
        Span::raw("   "),
        theme::muted_span("recv: "),
        Span::styled(
            format!(
                "{} / {}",
                theme::format_bytes(tick.recv, unit),
                theme::format_bytes(cum.recv, unit)
            ),
            Style::new().fg(theme::color::RECV),
        ),
    ])
}

fn detail_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme::color::ACCENT))
        .title_top(
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    title.to_string(),
                    Style::new()
                        .fg(theme::color::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
            .left_aligned(),
        )
}

fn table_title(state: &TuiState, tick: u64, interval: u64) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![
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
        Span::raw("  "),
        theme::muted_span("Interval:"),
        Span::raw(" "),
        theme::accent_span(format!("{}ms", interval).as_str()),
    ];
    if state.paused {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            "PAUSED".to_string(),
            Style::new()
                .fg(theme::color::HOT)
                .add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans).left_aligned()
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
            spans.push(theme::key_span("↑↓", false));
            spans.push(theme::label_span("move"));
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
            spans.push(theme::key_span("d", state.show_detail));
            spans.push(theme::label_span("details"));
            spans.push(theme::sep_span());
            spans.push(theme::key_span("p", state.paused));
            spans.push(theme::label_span(if state.paused {
                "resume"
            } else {
                "pause"
            }));
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
    let popup = centered_fixed(area, 52, 24);

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
        section_header("Navigation"),
        help_line("↑↓ jk", "Move the cursor (it tracks the process)"),
        help_line("d", "Toggle the per-process detail pane"),
        Line::raw(""),
        section_header("Filtering"),
        help_line("/", "Start or edit filter"),
        help_line("Tab", "Switch filter target (name ⇄ pid)"),
        help_line("Enter", "Apply filter"),
        help_line("BkSp", "Delete last character"),
        help_line("Esc", "Cancel input, or clear applied filter"),
        Line::raw(""),
        section_header("Other"),
        help_line("p", "Pause / resume the live feed"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{SortDir, SortKey};

    fn row(pid: u32, name: &str, tcp: (u64, u64), udp: (u64, u64)) -> StatsRow {
        StatsRow::new(
            pid,
            name,
            StatsBytes {
                sent: tcp.0,
                recv: tcp.1,
            },
            StatsBytes {
                sent: udp.0,
                recv: udp.1,
            },
            StatsBytes::default(),
            StatsBytes::default(),
        )
    }

    fn pids(rows: &[StatsRow], idx: &[usize]) -> Vec<u32> {
        idx.iter().map(|&i| rows[i].pid).collect()
    }

    #[test]
    fn sort_rows_by_sent_desc_and_asc() {
        let rows = [
            row(1, "a", (10, 0), (0, 0)),
            row(2, "b", (50, 0), (0, 0)),
            row(3, "c", (30, 0), (0, 0)),
        ];
        let mut idx: Vec<usize> = (0..rows.len()).collect();

        sort_rows(&rows, &mut idx, SortKey::Sent, SortDir::Desc);
        assert_eq!(pids(&rows, &idx), vec![2, 3, 1]);

        sort_rows(&rows, &mut idx, SortKey::Sent, SortDir::Asc);
        assert_eq!(pids(&rows, &idx), vec![1, 3, 2]);
    }

    #[test]
    fn sort_rows_by_recv_desc_and_asc() {
        let rows = [
            row(1, "a", (0, 10), (0, 0)),
            row(2, "b", (0, 50), (0, 0)),
            row(3, "c", (0, 30), (0, 0)),
        ];
        let mut idx: Vec<usize> = (0..rows.len()).collect();

        sort_rows(&rows, &mut idx, SortKey::Recv, SortDir::Desc);
        assert_eq!(pids(&rows, &idx), vec![2, 3, 1]);

        sort_rows(&rows, &mut idx, SortKey::Recv, SortDir::Asc);
        assert_eq!(pids(&rows, &idx), vec![1, 3, 2]);
    }

    #[test]
    fn sort_rows_by_total_desc_and_asc() {
        let rows = [
            row(1, "a", (10, 5), (0, 0)), // 15
            row(2, "b", (50, 5), (0, 0)), // 55
            row(3, "c", (30, 5), (0, 0)), // 35
        ];
        let mut idx: Vec<usize> = (0..rows.len()).collect();

        sort_rows(&rows, &mut idx, SortKey::Total, SortDir::Desc);
        assert_eq!(pids(&rows, &idx), vec![2, 3, 1]);

        sort_rows(&rows, &mut idx, SortKey::Total, SortDir::Asc);
        assert_eq!(pids(&rows, &idx), vec![1, 3, 2]);
    }

    #[test]
    fn sort_rows_by_name_asc_and_desc() {
        let rows = [
            row(1, "charlie", (0, 0), (0, 0)),
            row(2, "alpha", (0, 0), (0, 0)),
            row(3, "bravo", (0, 0), (0, 0)),
        ];
        let mut idx: Vec<usize> = (0..rows.len()).collect();

        sort_rows(&rows, &mut idx, SortKey::Name, SortDir::Asc);
        assert_eq!(pids(&rows, &idx), vec![2, 3, 1]);

        sort_rows(&rows, &mut idx, SortKey::Name, SortDir::Desc);
        assert_eq!(pids(&rows, &idx), vec![1, 3, 2]);
    }

    #[test]
    fn sort_rows_by_pid_asc_and_desc() {
        let rows = [
            row(30, "c", (0, 0), (0, 0)),
            row(10, "a", (0, 0), (0, 0)),
            row(20, "b", (0, 0), (0, 0)),
        ];
        let mut idx: Vec<usize> = (0..rows.len()).collect();

        sort_rows(&rows, &mut idx, SortKey::Pid, SortDir::Asc);
        assert_eq!(pids(&rows, &idx), vec![10, 20, 30]);

        sort_rows(&rows, &mut idx, SortKey::Pid, SortDir::Desc);
        assert_eq!(pids(&rows, &idx), vec![30, 20, 10]);
    }

    #[test]
    fn sort_rows_tie_breaks_by_pid_ascending_regardless_of_dir() {
        // Equal totals: the tie-break is always PID ascending.
        let rows = [
            row(2, "b", (10, 0), (0, 0)),
            row(1, "a", (10, 0), (0, 0)),
            row(3, "c", (10, 0), (0, 0)),
        ];
        let mut idx: Vec<usize> = (0..rows.len()).collect();

        sort_rows(&rows, &mut idx, SortKey::Total, SortDir::Desc);
        assert_eq!(pids(&rows, &idx), vec![1, 2, 3]);

        sort_rows(&rows, &mut idx, SortKey::Total, SortDir::Asc);
        assert_eq!(pids(&rows, &idx), vec![1, 2, 3]);
    }

    #[test]
    fn clamp_scroll_empty_view() {
        assert_eq!(clamp_scroll(0, 0, 5, 0), (0, 0));
        assert_eq!(clamp_scroll(99, 99, 5, 0), (0, 0));
    }

    #[test]
    fn clamp_scroll_zero_visible() {
        assert_eq!(clamp_scroll(3, 5, 0, 10), (3, 0));
    }

    #[test]
    fn clamp_scroll_all_fit_resets_offset_to_zero() {
        assert_eq!(clamp_scroll(2, 9, 5, 3), (2, 0));
    }

    #[test]
    fn clamp_scroll_scrolls_down_to_keep_selected_visible() {
        // 10 rows, 5 visible, selected 7 at offset 0 -> window becomes [3, 7].
        let (sel, off) = clamp_scroll(7, 0, 5, 10);
        assert_eq!((sel, off), (7, 3));
    }

    #[test]
    fn clamp_scroll_scrolls_up_to_keep_selected_visible() {
        // 10 rows, 5 visible, selected 1 at offset 8 -> window becomes [1, 5].
        let (sel, off) = clamp_scroll(1, 8, 5, 10);
        assert_eq!((sel, off), (1, 1));
    }

    #[test]
    fn clamp_scroll_prevents_overscroll_past_last_page() {
        // Offset is clamped to len - visible (last page), even when selected
        // is already on it.
        let (_, off) = clamp_scroll(9, 99, 5, 10);
        assert_eq!(off, 5);
    }

    #[test]
    fn clamp_scroll_clamps_selected_into_range() {
        assert_eq!(clamp_scroll(100, 0, 5, 10), (9, 5));
    }
}
