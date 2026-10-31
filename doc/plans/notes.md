# Implementation decisions

What the plans leave open, decided as the milestones go. Newest entries at the top of
each section.

## Core

- **`SetInputFocus` at `CurrentTime`** (M1). With event timestamps, a focus set earlier
  at `CurrentTime` moved the server's last-focus-change time past later requests' stamps,
  and X ignored them: f1 and f5 failed 10 times in 30 runs. Protocol messages
  (`WM_TAKE_FOCUS`, `WM_DELETE_WINDOW`) still carry the latest event timestamp.
- **Non-reparenting** (M1). No frames, so the border is the client's X border, no save
  set is needed and windows stay usable if sugoi exits.
- **A plain `poll` loop** (M1). `rustix::event::poll` over the X socket and a socket
  pair that wakes the loop for commands; `calloop` is not needed for two sources.
- **Commands through a `Handle`** (M1). An `mpsc` channel plus the wake socket; the
  tests use it now, the Ruby config (M2) and IPC (M7) will feed the same queue.
- **`log` and `env_logger`** (M1). To stderr, `info` by default, `RUST_LOG` overrides.
- **One Xvfb per test** (M1). `-displayfd 1` lets Xvfb pick a free display, so tests
  run in parallel; borders are checked by reading pixels back with `GetImage`, and
  pointer input goes through XTEST.

## Configuration

- **CRuby, embedded** (user, 2026-10-31). Full Ruby and gems through `magnus`, rather
  than mruby: the config is personal, so sandboxing matters less than the language and
  the bindings. The cost is a build tied to one libruby (0.1 plan, Risks).
- **One meaning per chord** (user, 2026-10-31). awesome's overlaps were mistakes:
  Mod-Shift-q kills the focused window, Mod-Shift-e quits, Mod-Ctrl-Return opens
  superkitty. Bindings never depend on whether a window has focus.

## Layouts

- **Tile, max and float only** (user, 2026-10-31). awesome's fair, bottom and top
  variants are dropped.
- **No swap with master** (user, 2026-10-31). Swapping with the next or previous window
  stays; Mod-Ctrl-Return goes to superkitty.

## Workspaces

- **Cycling by repeat** (user, 2026-10-31). Mod-n goes to n, then n+10, n+20, wrapping,
  decided by the current workspace rather than a timeout. Empty workspaces are not
  skipped, so a press always lands in the same place; polybar hides empty ones.
- **3 banks of 10** (user, 2026-10-31). 30 workspaces; Mod-0 is workspace 10, so the
  cycle step is 10 and Mod-3 pressed twice reads as workspace 13.
- **One monitor** (user, 2026-10-31). No multi-monitor support.

## Interop

- **polybar as the bar** (user, 2026-10-31). It covers workspaces, the title and the
  systray through EWMH; the window list is dropped, as the user does not use it.
- **dunst for notifications** (user, 2026-10-31). It replaces awesome's naughty, and
  sugoi reports config errors through it.

## Spawning

- **No workspace freezing** (user, 2026-10-31). Freezing hidden apps' cgroups stops
  background work and breaks network connections.
