# ttop

> A tmux-aware process tree with `top`-style CPU/memory views.

Sessions are tabs, each window a process tree, sorted by CPU or memory
(busiest first). Navigate, filter, and kill — from any pane.

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

## Run

Needs `tmux` and a Rust toolchain.

```sh
cargo run --release      # or: cargo install --path . && ttop
```

## Keys

Vim-style; arrows, `Home`/`End` and `PageUp`/`PageDown` work too.

| Key | Action |
| --- | --- |
| `j` / `k` | move (`Ctrl-d`/`Ctrl-u` half-page, `g`/`G` top/bottom) |
| `h` / `l`, `Tab` | switch session |
| `Enter` | fold / unfold window |
| `/` | filter by command or PID |
| `P` / `M` | sort by CPU / memory |
| `t` / `x` | SIGTERM / SIGKILL the selected process |
| `q` | quit |

CPU% is sampled between ~2s refreshes — the first frame reads ~0%, and a busy
process can exceed 100% on multiple cores.
