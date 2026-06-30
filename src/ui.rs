//! Rendering: session tabs, the window/process tree, and the footer.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Tabs};

use crate::app::{App, Row, Sort};

// Palette. Secondary ("dim") text needs enough contrast on a black background;
// `DIM_SEL` is a brighter variant used on the highlighted row so it stays
// readable on top of the selection bar.
const DIM: Color = Color::Indexed(245);
const DIM_SEL: Color = Color::Indexed(252);
const SELECT_BG: Color = Color::Indexed(238);

const HELP: &str = "j/k move · ^u/^d page · g/G ends · h/l session · Enter fold · ⇧Enter all · / filter · P cpu · M mem · t SIGTERM · x SIGKILL · q quit";

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // tabs
        Constraint::Length(1), // spacer
        Constraint::Min(0),    // body
        Constraint::Length(1), // footer
    ])
    .horizontal_margin(1)
    .split(f.area());

    draw_tabs(f, app, chunks[0]);
    draw_body(f, app, chunks[2]);
    draw_footer(f, app, chunks[3]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let label = "Sessions: ";
    let label_w = label.len() as u16;
    let label_area = Rect::new(area.x, area.y, label_w.min(area.width), area.height);
    f.render_widget(
        Paragraph::new(Span::styled(
            label,
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
        )),
        label_area,
    );

    let titles: Vec<Line> = app
        .sessions
        .iter()
        .map(|s| {
            if s.current {
                Line::from(vec![
                    Span::raw(" "),
                    Span::raw(s.name.as_str()),
                    Span::styled("*", Style::default().fg(Color::Green)),
                    Span::raw(" "),
                ])
            } else {
                Line::from(format!(" {} ", s.name))
            }
        })
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.selected_tab)
        .divider("")
        .padding("", "")
        .style(Style::default().fg(DIM))
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .bg(SELECT_BG)
                .add_modifier(Modifier::BOLD),
        );
    let tabs_area = Rect::new(
        area.x + label_w,
        area.y,
        area.width.saturating_sub(label_w),
        area.height,
    );
    f.render_widget(tabs, tabs_area);
}

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    let parts = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);
    app.set_viewport(parts[1].height as usize);

    let sort_label = match app.sort {
        Sort::Cpu => "CPU",
        Sort::Mem => "MEM",
    };
    let header = Line::from(vec![
        Span::styled(
            format!("  {:>7}  {:>6}  {:>8}  COMMAND", "PID", "CPU%", "MEM"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("   (sort: {sort_label} ▼)"),
            Style::default().fg(DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(header), parts[0]);

    let rows = app.rows();
    if rows.is_empty() {
        let msg = if app.sessions.is_empty() {
            "No tmux sessions found."
        } else {
            "No matching processes."
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {msg}"),
                Style::default().fg(DIM),
            ))),
            parts[1],
        );
        return;
    }

    let width = parts[1].width as usize;
    let sel = app.selected.min(rows.len().saturating_sub(1));
    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| render_row(r, width, i == sel))
        .collect();
    // A uniform background bar (not REVERSED) so per-column colors are kept and
    // the whole selected row gets one consistent highlight.
    let list = List::new(items).highlight_style(Style::default().bg(SELECT_BG));
    let mut state = ListState::default();
    state.select(Some(sel));
    f.render_stateful_widget(list, parts[1], &mut state);
}

fn render_row(row: &Row, width: usize, selected: bool) -> ListItem<'static> {
    let dim = Style::default().fg(if selected { DIM_SEL } else { DIM });
    match row {
        Row::Window {
            key: (_, index),
            name,
            active,
            collapsed,
            count,
            cpu,
            mem,
        } => {
            let arrow = if *collapsed { "▸ " } else { "▾ " };
            let name_color = if *active { Color::Green } else { Color::Cyan };
            let name_style = Style::default().fg(name_color).add_modifier(Modifier::BOLD);
            let line = Line::from(vec![
                Span::styled(arrow, Style::default().fg(Color::Cyan)),
                Span::styled(format!("[{index}] {name}"), name_style),
                Span::styled(
                    format!("   {count} proc · {cpu:.1}% · {}", format_bytes(*mem)),
                    dim,
                ),
            ]);
            ListItem::new(line)
        }
        Row::Proc { proc: p, prefix } => {
            let cols = 2 + 7 + 2 + 6 + 2 + 8 + 2;
            let avail = width.saturating_sub(cols).max(4);
            let max_cmd = avail.saturating_sub(prefix.chars().count()).max(4);
            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{:>7}", p.pid), dim),
                Span::raw("  "),
                Span::styled(format!("{:>6.1}", p.cpu), cpu_style(p.cpu)),
                Span::raw("  "),
                Span::styled(format!("{:>8}", format_bytes(p.mem)), mem_style(p.mem)),
                Span::raw("  "),
                Span::styled(prefix.clone(), dim),
                Span::raw(truncate(&p.command, max_cmd)),
            ]);
            ListItem::new(line)
        }
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let dim = Style::default().fg(DIM);
    let yellow = Style::default().fg(Color::Yellow);
    let line = if app.filtering {
        Line::from(vec![
            Span::styled("/", yellow.add_modifier(Modifier::BOLD)),
            Span::raw(app.filter.as_str()),
            Span::styled("▏", yellow),
            Span::styled("   Enter: apply · Esc: clear", dim),
        ])
    } else if let Some(status) = &app.status {
        Line::from(Span::styled(status.as_str(), yellow))
    } else {
        let mut spans = Vec::new();
        if !app.filter.is_empty() {
            spans.push(Span::styled(format!("filter:{} · ", app.filter), yellow));
        }
        spans.push(Span::styled(HELP, dim));
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
}

fn cpu_style(cpu: f32) -> Style {
    if cpu >= 50.0 {
        Style::default().fg(Color::Red)
    } else if cpu >= 10.0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}

fn mem_style(mem: u64) -> Style {
    const MB: u64 = 1024 * 1024;
    if mem >= 1024 * MB {
        Style::default().fg(Color::Red)
    } else if mem >= 256 * MB {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}

fn format_bytes(b: u64) -> String {
    const K: f64 = 1024.0;
    let b = b as f64;
    if b < K {
        format!("{b:.0}B")
    } else if b < K * K {
        format!("{:.0}K", b / K)
    } else if b < K * K * K {
        format!("{:.0}M", b / (K * K))
    } else {
        format!("{:.1}G", b / (K * K * K))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}…")
}
