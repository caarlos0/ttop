//! Query tmux for its session/window/pane structure.

use std::process::Command;

/// A single tmux pane and the window/session it belongs to.
pub struct Pane {
    pub session: String,
    pub window_index: u32,
    pub window_name: String,
    pub window_active: bool,
    /// PID of the process tmux started in the pane (its shell, usually).
    pub pane_pid: u32,
}

/// The full tmux structure plus which session is currently focused.
pub struct TmuxInfo {
    pub panes: Vec<Pane>,
    /// Session names in tmux's own order.
    pub sessions: Vec<String>,
    pub current_session: Option<String>,
}

/// Ask tmux about every session, window and pane.
pub fn query() -> std::io::Result<TmuxInfo> {
    let current_session = run(&["display-message", "-p", "#{session_name}"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let sessions = run(&["list-sessions", "-F", "#{session_name}"])?
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let fmt = "#{session_name}\t#{window_index}\t#{window_name}\t#{window_active}\t#{pane_pid}";
    let panes_raw = run(&["list-panes", "-a", "-F", fmt])?;
    let mut panes = Vec::new();
    for line in panes_raw.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        let Ok(pane_pid) = f[4].trim().parse::<u32>() else {
            continue;
        };
        panes.push(Pane {
            session: f[0].to_string(),
            window_index: f[1].trim().parse().unwrap_or(0),
            window_name: f[2].to_string(),
            window_active: f[3].trim() == "1",
            pane_pid,
        });
    }

    Ok(TmuxInfo {
        panes,
        sessions,
        current_session,
    })
}

fn run(args: &[&str]) -> std::io::Result<String> {
    let out = Command::new("tmux").args(args).output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(format!("tmux {args:?} failed")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
