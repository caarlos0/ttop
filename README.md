# ttop

A tiny terminal UI that shows the processes running inside every **tmux** pane,
grouped by session and window — like `htop`, but scoped to your tmux.

```
┌ ttop · tmux process top ───────────────────────────────────────────────┐
│ ● copilot-agent-runtime │   default │   s                              │
└────────────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────────────┐
│      PID    CPU%       MEM  COMMAND   (sort: CPU ▼)                     │
│▾ [4] ttop   8 proc · 21.1% · 457M                                       │
│     8149    16.8      361M  node /Users/you/.bin/copilot --yolo         │
│     3377     0.0        8M  -fish                                       │
│▾ [3] slack-pr-size   8 proc · 0.1% · 310M                              │
│    65143     0.1      232M  node /Users/you/.bin/copilot --yolo         │
│    43225     0.0       33M  git show @                                  │
└────────────────────────────────────────────────────────────────────────┘
 ↑↓ move · ←→/Tab session · Enter fold · / filter · P cpu · M mem · k SIGTERM · x SIGKILL · q quit
```

## Features

- Every tmux session is a **tab**; the session you're currently in is pre-selected.
- Processes are grouped into a `window → processes` **tree** (a window's panes are
  folded together).
- **Sort** by CPU or memory.
- **Filter**, navigate, and **kill** processes without leaving the tree.

## Requirements

- `tmux` on your `PATH` (ttop reads the running server).
- A Rust toolchain to build.

## Install & run

```sh
cargo run --release
# or install it onto your PATH:
cargo install --path .
ttop
```

Run it from inside tmux so the current session is highlighted (it works from
outside too).

## Keybindings

| Key | Action |
| --- | --- |
| `↑` / `↓` | Move selection |
| `←` / `→`, `h` / `l`, `Tab` / `Shift-Tab` | Switch session tab |
| `Enter` / `Space` | Fold / unfold a window |
| `/` | Filter by command or PID — `Enter` applies, `Esc` clears |
| `P` | Sort by CPU |
| `M` | Sort by memory |
| `k` | Send `SIGTERM` to the selected process |
| `x` | Send `SIGKILL` to the selected process |
| `q` / `Ctrl-C` | Quit |

Sort and kill keys follow `top`/`htop` conventions (`P` = CPU, `M` = memory).

## How it works

- `tmux list-panes -a` provides the session/window/pane layout and each pane's
  root PID.
- [`sysinfo`](https://crates.io/crates/sysinfo) supplies per-process CPU%,
  memory and parent PID; every process is attached to the window of its nearest
  pane-root ancestor.
- The view refreshes every couple of seconds.

CPU percentages are computed from the delta between refreshes, so the first
frame reads ~0%. On multi-core machines a busy process can exceed 100%.
