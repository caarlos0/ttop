//! Rendering: session tabs, the window/process tree, and the footer.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};

use crate::app::{App, Row, Sort};

const HELP: &str = " ↑↓ move · ←→/Tab session · Enter fold · / filter · P cpu · M mem · k SIGTERM · x SIGKILL · q quit";

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(f.area());

    draw_tabs(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = app
        .sessions
        .iter()
        .map(|s| {
            if s.current {
                Line::from(vec![
                    Span::styled("● ", Style::default().fg(Color::Green)),
                    Span::raw(s.name.as_str()),
                ])
            } else {
                Line::from(format!("  {}", s.name))
            }
        })
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.selected_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    // No border; inset one column so the tabs line up with the body's content.
    let area = Rect::new(
        area.x + 1,
        area.y,
        area.width.saturating_sub(1),
        area.height,
    );
    f.render_widget(tabs, area);
}

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" ttop · tmux process top ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let parts = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(inner);

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
            Style::default().fg(Color::DarkGray),
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
                Style::default().fg(Color::DarkGray),
            ))),
            parts[1],
        );
        return;
    }

    let width = parts[1].width as usize;
    let items: Vec<ListItem> = rows.iter().map(|r| render_row(r, width)).collect();
    // A uniform background bar (not REVERSED) so per-column colors are kept and
    // the whole selected row gets one consistent highlight.
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::Indexed(238))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(app.selected.min(rows.len().saturating_sub(1))));
    f.render_stateful_widget(list, parts[1], &mut state);
}

fn render_row(row: &Row, width: usize) -> ListItem<'static> {
    match row {
        Row::Window {
            index,
            name,
            active,
            collapsed,
            count,
            cpu,
            mem,
            ..
        } => {
            let arrow = if *collapsed { "▸ " } else { "▾ " };
            let name_style = if *active {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            };
            let line = Line::from(vec![
                Span::styled(arrow, Style::default().fg(Color::Cyan)),
                Span::styled(format!("[{index}] {name}"), name_style),
                Span::styled(
                    format!("   {count} proc · {cpu:.1}% · {}", format_bytes(*mem)),
                    Style::default().fg(Color::DarkGray),
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
                Span::styled(
                    format!("{:>7}", p.pid),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(format!("{:>6.1}", p.cpu), cpu_style(p.cpu)),
                Span::raw("  "),
                Span::styled(format!("{:>8}", format_bytes(p.mem)), mem_style(p.mem)),
                Span::raw("  "),
                Span::styled(prefix.clone(), Style::default().fg(Color::DarkGray)),
                Span::raw(truncate(&p.command, max_cmd)),
            ]);
            ListItem::new(line)
        }
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let dim = Style::default().fg(Color::DarkGray);
    let line = if app.filtering {
        Line::from(vec![
            Span::styled(
                " /",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(app.filter.as_str()),
            Span::styled("▏", Style::default().fg(Color::Yellow)),
            Span::styled("   Enter: apply · Esc: clear", dim),
        ])
    } else if let Some(status) = &app.status {
        Line::from(Span::styled(
            format!(" {status}"),
            Style::default().fg(Color::Yellow),
        ))
    } else {
        let mut spans = Vec::new();
        if !app.filter.is_empty() {
            spans.push(Span::styled(
                format!(" filter:{} ·", app.filter),
                Style::default().fg(Color::Yellow),
            ));
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
