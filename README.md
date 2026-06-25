# ttop

A tiny terminal UI that shows the processes running inside every **tmux** pane,
grouped by session and window — like `htop`, but scoped to your tmux.

```
 Sessions:  ● dev │   infra │   notes

       PID    CPU%       MEM  COMMAND   (sort: CPU ▼)
 ▾ [2] api   4 proc · 14.2% · 354M
      5990     0.0        9M  └─ -fish
      6021    13.7      320M     └─ node server.js
      6044     0.5       21M        ├─ esbuild
      6050     0.0        4M        └─ tsc --watch
 ▾ [1] editor   2 proc · 1.8% · 89M
      4775     0.0        9M  └─ -fish
      4810     1.8       80M     └─ nvim
 j/k move · ^u/^d page · g/G ends · h/l session · Enter fold · / filter · P cpu · M mem · t SIGTERM · x SIGKILL · q quit
```

## Features

- Every tmux session is a **tab**; the session you're currently in is pre-selected.
- Processes are grouped into a `window → processes` **tree** (a window's panes are
  folded together).
- **Sort** by CPU or memory — orders both the processes and the windows
  (busiest window first).
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
| `j` / `k`, `↓` / `↑` | Move selection |
| `Ctrl-d` / `Ctrl-u` | Half-page down / up |
| `PgDn` / `PgUp` | Page down / up |
| `h` / `l`, `←` / `→`, `Tab` / `Shift-Tab` | Switch session tab |
| `g` / `G`, `Home` / `End` | Jump to top / bottom |
| `Enter` / `Space` | Fold / unfold a window |
| `/` | Filter by command or PID — `Enter` applies, `Esc` clears |
| `P` | Sort by CPU |
| `M` | Sort by memory |
| `t` | Send `SIGTERM` to the selected process |
| `x` | Send `SIGKILL` to the selected process |
| `q` / `Ctrl-C` | Quit |

Navigation is vim-style (`hjkl`, `g`/`G`, `Ctrl-d`/`Ctrl-u`); arrow/Home/End/Page
keys work too. Sort keys follow `top` conventions (`P` = CPU, `M` = memory).

## How it works

- `tmux list-panes -a` provides the session/window/pane layout and each pane's
  root PID.
- [`sysinfo`](https://crates.io/crates/sysinfo) supplies per-process CPU%,
  memory and parent PID; every process is attached to the window of its nearest
  pane-root ancestor.
- The view refreshes every couple of seconds.

CPU percentages are computed from the delta between refreshes, so the first
frame reads ~0%. On multi-core machines a busy process can exceed 100%.
