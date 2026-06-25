//! Collect process information and fold it into the tmux session tree.

use std::collections::HashMap;

use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

use crate::tmux::TmuxInfo;

/// A single process attached to a tmux pane.
#[derive(Clone)]
pub struct Proc {
    pub pid: u32,
    pub cpu: f32,
    pub mem: u64,
    pub command: String,
}

/// A tmux window with all the processes running across its panes.
#[derive(Clone)]
pub struct Window {
    pub index: u32,
    pub name: String,
    pub active: bool,
    pub procs: Vec<Proc>,
}

/// A tmux session and its windows.
#[derive(Clone)]
pub struct Session {
    pub name: String,
    pub current: bool,
    pub windows: Vec<Window>,
}

/// Owns the `sysinfo` system so CPU usage can be computed across refreshes.
pub struct Collector {
    sys: System,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        // Two refreshes a short interval apart so the first frame already shows
        // meaningful CPU percentages instead of zeros.
        sys.refresh_processes(ProcessesToUpdate::All, true);
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_processes(ProcessesToUpdate::All, true);
        Self { sys }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
    }

    pub fn process(&self, pid: u32) -> Option<&Process> {
        self.sys.process(Pid::from_u32(pid))
    }

    fn processes(&self) -> &HashMap<Pid, Process> {
        self.sys.processes()
    }
}

fn command_string(p: &Process) -> String {
    let cmd = p.cmd();
    if cmd.is_empty() {
        p.name().to_string_lossy().into_owned()
    } else {
        cmd.iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Build the session tree by attaching every process to the window of its
/// nearest pane-root ancestor.
pub fn build_sessions(collector: &Collector, info: &TmuxInfo) -> Vec<Session> {
    let procs = collector.processes();

    // pane root pid -> (session, window index)
    let mut pane_root: HashMap<u32, (String, u32)> = HashMap::new();
    // (session, window index) -> (window name, active), keeping first-seen order.
    let mut win_meta: HashMap<(String, u32), (String, bool)> = HashMap::new();
    let mut win_order: Vec<(String, u32)> = Vec::new();
    for p in &info.panes {
        pane_root.insert(p.pane_pid, (p.session.clone(), p.window_index));
        let key = (p.session.clone(), p.window_index);
        if !win_meta.contains_key(&key) {
            win_meta.insert(key.clone(), (p.window_name.clone(), p.window_active));
            win_order.push(key);
        }
    }

    let mut buckets: HashMap<(String, u32), Vec<Proc>> = HashMap::new();
    for (pid, proc_) in procs.iter() {
        // Walk up the parent chain until we hit a pane root (or give up).
        let mut cur = Some(*pid);
        let mut hops = 0;
        let mut found: Option<(String, u32)> = None;
        while let Some(c) = cur {
            if let Some(key) = pane_root.get(&c.as_u32()) {
                found = Some(key.clone());
                break;
            }
            cur = procs.get(&c).and_then(|p| p.parent());
            hops += 1;
            if hops > 128 {
                break;
            }
        }
        if let Some(key) = found {
            buckets.entry(key).or_default().push(Proc {
                pid: pid.as_u32(),
                cpu: proc_.cpu_usage(),
                mem: proc_.memory(),
                command: command_string(proc_),
            });
        }
    }

    let mut sessions = Vec::new();
    for sname in &info.sessions {
        let mut windows: Vec<Window> = win_order
            .iter()
            .filter(|(s, _)| s == sname)
            .map(|key| {
                let (name, active) = win_meta.get(key).cloned().unwrap_or_default();
                let procs = buckets.remove(key).unwrap_or_default();
                Window {
                    index: key.1,
                    name,
                    active,
                    procs,
                }
            })
            .collect();
        windows.sort_by_key(|w| w.index);
        sessions.push(Session {
            name: sname.clone(),
            current: info.current_session.as_deref() == Some(sname.as_str()),
            windows,
        });
    }
    sessions
}
