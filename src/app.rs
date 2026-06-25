//! Application state: navigation, filtering, sorting and kill actions.

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sysinfo::Signal;

use crate::model::{self, Collector, Proc, Session};
use crate::tmux;

/// Which metric the process lists are sorted by.
#[derive(Clone, Copy)]
pub enum Sort {
    Cpu,
    Mem,
}

/// A single visible line in the tree: either a window header or a process.
pub enum Row {
    Window {
        key: (String, u32),
        index: u32,
        name: String,
        active: bool,
        collapsed: bool,
        count: usize,
        cpu: f32,
        mem: u64,
    },
    /// A process. `prefix` holds the rendered tree branch (e.g. `"│  └─ "`).
    Proc { proc: Proc, prefix: String },
}

pub struct App {
    collector: Collector,
    pub sessions: Vec<Session>,
    pub selected_tab: usize,
    pub sort: Sort,
    collapsed: HashSet<(String, u32)>,
    pub filter: String,
    pub filtering: bool,
    pub selected: usize,
    pub status: Option<String>,
    status_ttl: u8,
    pub should_quit: bool,
    /// Visible list height from the last render, used for page scrolling.
    viewport: Cell<usize>,
}

impl App {
    pub fn new() -> std::io::Result<Self> {
        let collector = Collector::new();
        let info = tmux::query()?;
        let sessions = model::build_sessions(&collector, &info);
        let selected_tab = sessions.iter().position(|s| s.current).unwrap_or(0);
        Ok(Self {
            collector,
            sessions,
            selected_tab,
            sort: Sort::Cpu,
            collapsed: HashSet::new(),
            filter: String::new(),
            filtering: false,
            selected: 0,
            status: None,
            status_ttl: 0,
            should_quit: false,
            viewport: Cell::new(20),
        })
    }

    /// Re-read processes and tmux structure, keeping the user roughly in place.
    pub fn refresh(&mut self) {
        self.collector.refresh();
        if let Ok(info) = tmux::query() {
            let keep = self.sessions.get(self.selected_tab).map(|s| s.name.clone());
            self.sessions = model::build_sessions(&self.collector, &info);
            if let Some(name) = keep
                && let Some(pos) = self.sessions.iter().position(|s| s.name == name)
            {
                self.selected_tab = pos;
            }
            if self.selected_tab >= self.sessions.len() {
                self.selected_tab = self.sessions.len().saturating_sub(1);
            }
        }
        self.clamp();
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status = None;
            }
        }
    }

    /// Flatten the selected session into the visible rows: a header per window
    /// followed by its processes laid out as a tree, honoring the active filter,
    /// sort order and collapse state.
    pub fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        let Some(session) = self.sessions.get(self.selected_tab) else {
            return out;
        };
        let needle = self.filter.to_lowercase();
        let filtering = !needle.is_empty();
        let sort = self.sort;

        for w in &session.windows {
            let by_pid: HashMap<u32, &Proc> = w.procs.iter().map(|p| (p.pid, p)).collect();

            // When filtering, keep matching processes plus all their ancestors so
            // each match is still shown inside its branch of the tree.
            let keep: Option<HashSet<u32>> = if filtering {
                let mut keep = HashSet::new();
                for p in &w.procs {
                    let hit = p.command.to_lowercase().contains(&needle)
                        || p.pid.to_string().contains(&needle);
                    if !hit {
                        continue;
                    }
                    let mut cur = Some(p.pid);
                    while let Some(pid) = cur {
                        if !keep.insert(pid) {
                            break;
                        }
                        cur = by_pid
                            .get(&pid)
                            .map(|p| p.ppid)
                            .filter(|ppid| by_pid.contains_key(ppid));
                    }
                }
                if keep.is_empty() {
                    continue;
                }
                Some(keep)
            } else {
                None
            };
            let included = |pid: u32| keep.as_ref().is_none_or(|k| k.contains(&pid));

            // Build the parent -> children forest over the included processes.
            // A process is a root when its parent isn't part of this window (i.e.
            // it is one of the pane's top-level processes).
            let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
            let mut roots: Vec<u32> = Vec::new();
            for p in &w.procs {
                if !included(p.pid) {
                    continue;
                }
                if by_pid.contains_key(&p.ppid) && included(p.ppid) {
                    children.entry(p.ppid).or_default().push(p.pid);
                } else {
                    roots.push(p.pid);
                }
            }
            roots.sort_by(|a, b| cmp_proc(sort, by_pid[a], by_pid[b]));
            for kids in children.values_mut() {
                kids.sort_by(|a, b| cmp_proc(sort, by_pid[a], by_pid[b]));
            }

            let key = (session.name.clone(), w.index);
            let collapsed = !filtering && self.collapsed.contains(&key);
            let cpu: f32 = w
                .procs
                .iter()
                .filter(|p| included(p.pid))
                .map(|p| p.cpu)
                .sum();
            let mem: u64 = w
                .procs
                .iter()
                .filter(|p| included(p.pid))
                .map(|p| p.mem)
                .sum();
            let count = w.procs.iter().filter(|p| included(p.pid)).count();
            out.push(Row::Window {
                key,
                index: w.index,
                name: w.name.clone(),
                active: w.active,
                collapsed,
                count,
                cpu,
                mem,
            });
            if !collapsed {
                let n = roots.len();
                for (i, &r) in roots.iter().enumerate() {
                    emit_tree(r, "", i + 1 == n, &children, &by_pid, &mut out);
                }
            }
        }
        out
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filtering = false;
                    self.filter.clear();
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.selected = 0;
                }
                _ => {}
            }
            self.clamp();
            return;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.selected += 1,
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.selected = self.selected.saturating_sub(self.half_page())
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.selected += self.half_page()
            }
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(self.viewport.get()),
            KeyCode::PageDown => self.selected += self.viewport.get().max(1),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => self.prev_tab(),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.next_tab(),
            KeyCode::Home | KeyCode::Char('g') => self.selected = 0,
            KeyCode::End | KeyCode::Char('G') => {
                self.selected = self.rows().len().saturating_sub(1)
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_fold(),
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Esc => self.filter.clear(),
            KeyCode::Char('p') | KeyCode::Char('P') => self.sort = Sort::Cpu,
            KeyCode::Char('m') | KeyCode::Char('M') => self.sort = Sort::Mem,
            KeyCode::Char('x') => self.kill_selected(Signal::Kill, "SIGKILL"),
            KeyCode::Char('t') => self.kill_selected(Signal::Term, "SIGTERM"),
            _ => {}
        }
        self.clamp();
    }

    fn clamp(&mut self) {
        let n = self.rows().len();
        if self.selected >= n {
            self.selected = n.saturating_sub(1);
        }
    }

    /// Record the visible list height (called by the renderer each frame) so
    /// page scrolling can move by a real page.
    pub fn set_viewport(&self, rows: usize) {
        self.viewport.set(rows);
    }

    fn half_page(&self) -> usize {
        (self.viewport.get() / 2).max(1)
    }

    fn next_tab(&mut self) {
        if !self.sessions.is_empty() {
            self.selected_tab = (self.selected_tab + 1) % self.sessions.len();
            self.selected = 0;
        }
    }

    fn prev_tab(&mut self) {
        if !self.sessions.is_empty() {
            let n = self.sessions.len();
            self.selected_tab = (self.selected_tab + n - 1) % n;
            self.selected = 0;
        }
    }

    fn toggle_fold(&mut self) {
        let rows = self.rows();
        if let Some(Row::Window { key, collapsed, .. }) = rows.get(self.selected) {
            let key = key.clone();
            let was = *collapsed;
            drop(rows);
            if was {
                self.collapsed.remove(&key);
            } else {
                self.collapsed.insert(key);
            }
        }
    }

    fn kill_selected(&mut self, sig: Signal, label: &str) {
        let rows = self.rows();
        let target = match rows.get(self.selected) {
            Some(Row::Proc { proc, .. }) => Some((proc.pid, short(&proc.command))),
            _ => None,
        };
        drop(rows);
        let Some((pid, name)) = target else {
            return;
        };
        let result = self.collector.process(pid).map(|p| p.kill_with(sig));
        self.status = Some(match result {
            Some(Some(true)) => format!("Sent {label} → {pid} {name}"),
            Some(Some(false)) => format!("Failed to send {label} → {pid}"),
            Some(None) => format!("{label} unsupported on this platform"),
            None => format!("PID {pid} no longer exists"),
        });
        self.status_ttl = 3;
    }
}

/// Short, human-friendly process name for status messages.
fn short(cmd: &str) -> String {
    let first = cmd.split_whitespace().next().unwrap_or(cmd);
    let base = first.rsplit('/').next().unwrap_or(first);
    base.chars().take(40).collect()
}

/// Order two processes by the active sort metric, descending, breaking ties
/// with the other metric.
fn cmp_proc(sort: Sort, a: &Proc, b: &Proc) -> Ordering {
    match sort {
        Sort::Cpu => b
            .cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(Ordering::Equal)
            .then(b.mem.cmp(&a.mem)),
        Sort::Mem => b
            .mem
            .cmp(&a.mem)
            .then(b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Equal)),
    }
}

/// Depth-first walk that emits a process and its children as tree rows,
/// building the `├─`/`└─`/`│` branch prefix as it descends.
fn emit_tree(
    pid: u32,
    prefix: &str,
    last: bool,
    children: &HashMap<u32, Vec<u32>>,
    by_pid: &HashMap<u32, &Proc>,
    out: &mut Vec<Row>,
) {
    let branch = if last { "└─ " } else { "├─ " };
    out.push(Row::Proc {
        proc: by_pid[&pid].clone(),
        prefix: format!("{prefix}{branch}"),
    });
    let child_prefix = format!("{prefix}{}", if last { "   " } else { "│  " });
    if let Some(kids) = children.get(&pid) {
        let n = kids.len();
        for (i, &c) in kids.iter().enumerate() {
            emit_tree(c, &child_prefix, i + 1 == n, children, by_pid, out);
        }
    }
}
