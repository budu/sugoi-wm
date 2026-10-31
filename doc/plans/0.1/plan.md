# sugoi-wm 0.1 plan

Status: draft, 2026-10-31.

Version 0.1 replaces awesome as the daily window manager on a single-monitor X11 session:
a Rust core, a Ruby configuration, apps spawned in their own cgroup scopes, three
layouts (tile, max, float) and workspaces that cycle past 9 when Mod-n is pressed again.
M1 comes first: every later milestone builds on its window model and its X test harness.

## Evidence

| Finding                                                               | Source          |
|-----------------------------------------------------------------------|-----------------|
| The current setup is awesome 4.3, with 1,019 lines of Lua in 14 files | awesome config  |
| It binds 39 keys (30 global, 9 per client) plus 36 tag keys (4 × 9)   | keybindings.lua |
| It has 6 mouse bindings: 3 on the root window, 3 on clients           | mousebindings   |
| It has 9 rules: 1 default, 8 by class; 1 needs a query (2nd browser)  | rules.lua       |
| The Zed `setmaster` rule matches on `instance` and is marked FIXME    | rules.lua       |
| Two chords overlap: Mod-Shift-q (quit, kill) and Mod-Ctrl-Return      | keybindings.lua |
| 7 layouts rotate; the first is `tile.left` (master on the right)      | layouts.lua     |
| Apps spawn via `systemd-run --user --scope --slice=app-<name>.slice`  | functions.lua   |
| Tag keys use keycodes (#10–#18), and a keymap toggle exists (Mod-Tab) | keybindings.lua |
| The wibar holds taglist, tasklist, systray, clock, 7 status widgets   | wibar.lua       |
| awesome (naughty) is the notification daemon; dunst is not installed  | rc.lua, which   |
| awesome uses 68.8 MiB RSS, 0.6 % CPU (917 s over 42.7 h, widgets on)  | ps, 2026-10-31  |
| The pacman Ruby is 3.4.10, but the Ruby on `PATH` is asdf's 4.0.1     | pacman, ruby -v |
| Rust 1.98.1, systemd 262, cgroup v2 with cpu, memory and pids         | host            |
| Neither Xephyr nor Xvfb is installed; xdotool is                      | which, pacman   |
| Decided: CRuby embedded, tile/max/float, one monitor, no freezing     | user            |

- **Already there.** cgroup spawning per app works today through `systemd-run`, so M6
  ports a proven behavior rather than inventing one. External tools cover the menu
  (rofi, dmenu) and the confirmation prompts (dmenu).

## Scope

- **Core:** manage, focus and unmanage windows on one screen, with borders and sloppy
  focus.
- **Configuration:** embedded CRuby with a DSL for keys, mouse, spawn, rules and hooks,
  reloaded live.
- **Layouts:** tile (master and stack), max and float, with mouse move and resize.
- **Workspaces:** Mod-n goes to n, then n+10, n+20, wrapping; Mod-Shift-n moves the window.
- **Spawning:** every spawned app runs in a systemd scope, and windows map back to it.
- **Interop:** enough EWMH for an external bar, plus an IPC socket and `sugoictl`.

## Measures

- **Parity.** Each of the 39 keys, 36 tag keys, 6 mouse bindings and 9 rules in the
  awesome config has a sugoi equivalent or a line in `parity.md` saying why it is
  dropped.
- **Config safety.** Tests load a config with a syntax error, one whose hook raises and
  one whose hook loops; the WM keeps running on the previous config and reports the
  error.
- **Tests.** An integration suite drives the WM under Xvfb from `cargo test`, one case
  per behavior, and passes at every milestone close.
- **Stability.** The user runs sugoi as the session WM for the agreed period (open
  question) with no panic and no lost window.
- **Footprint.** RSS and idle CPU are measured at the release as the baseline for 0.2;
  no target in 0.1. awesome's 68.8 MiB includes its widgets, so it is a reference only.

## Principles

- **Rust owns the state.** Ruby configures and reacts; it never holds the window list.
  Events without a registered hook never enter Ruby.
- **Never die on config.** A config error keeps the previous config live and is shown to
  the user. The WM starts with built-in defaults if no config ever loaded.
- **One screen.** No multi-monitor abstraction: one root, one workspace visible.
- **Standards over widgets.** No bar, widgets, titlebars or notifications in the WM;
  EWMH and IPC let other programs do that.
- **Keycodes for position keys.** Number keys bind by keycode so they survive the keymap
  toggle.

## Milestone workflow

Research, specification first, contract cases, implementation, one review and one
re-review, long runs at the close, and the close with the hours. There are no
downstream clients and no benchmarks in 0.1; the footprint is measured once, at the
release.

## Milestones

| Milestone           | Contents                                                         |
|---------------------|------------------------------------------------------------------|
| M1 Core             | Become the WM, manage windows, focus, borders, Xvfb test harness |
| M2 Ruby config      | Embedded CRuby, key and mouse DSL, spawn, actions, live reload   |
| M3 Layouts          | Tile, max and float; mouse move and resize; window states        |
| M4 Workspaces       | Mod-n cycling over banks, move window, previous workspace        |
| M5 Rules and hooks  | Rules by class and instance, hooks with a query API              |
| M6 cgroup spawning  | Scopes per app, window to unit mapping, kill by unit             |
| M7 EWMH and IPC     | Bar-facing EWMH, struts, IPC socket, `sugoictl`                  |
| M8 Release          | Daily-drive period, footprint, parity check, the 0.1 report      |

### M1 Core

- **Crate.** One Cargo workspace: `sugoi` (the WM) and `sugoictl` (empty until M7).
  `x11rb` for X11.
- **WM.** Take `SubstructureRedirect` on the root and fail clearly if another WM runs;
  adopt existing windows at startup; handle map, unmap, destroy and configure requests.
- **Focus.** Sloppy focus on enter, click to focus and raise, `WM_TAKE_FOCUS` and
  `WM_DELETE_WINDOW` per ICCCM; borders with focused and normal colors.
- **Harness.** Integration tests start Xvfb and the WM, open test windows (a small X
  client in the test crate), and assert on X state. Needs `xorg-server-xvfb`; Xephyr
  for manual runs.
- **Left to research.** Event loop shape (`calloop` or a plain poll loop) and
  logging.

### M2 Ruby config

- **Embedding.** CRuby through `magnus` with the `embed` feature, on the main thread.
  Which libruby to link (pacman 3.4 or asdf 4.0) is settled first; see Risks.
- **DSL.** Keys and mouse buttons with modifiers, by keysym or keycode; actions (focus,
  swap, kill, quit, restart, reload, spawn); a Ruby block as an action.
- **Spawn.** Through `systemd-run --user --scope` as today, so the config ports at once;
  M6 replaces the mechanism.
- **Reload.** Mod-Ctrl-r evaluates the config into a new set of bindings and swaps it in
  only on success; errors go to the log and to `notify-send` if a daemon exists.
- **Guard.** A watchdog stops a hook that runs past a time limit (value set by research).
- **Artifact.** `parity.md` starts here: every awesome binding and rule, with its
  status.

### M3 Layouts

- **Tile.** Master and stack, master side configurable (today's default puts it on the
  right), master width factor (Mod-h and Mod-;), per-client height factor, swap with
  the master, new windows join the stack.
- **Max and float.** Max fills the work area; float keeps the window's geometry and
  places new windows centered and on screen.
- **States.** Fullscreen, maximize, minimize and restore, on top, center, and the 70 %
  centered resize from the current config.
- **Mouse.** Mod-drag moves and Mod-right-drag resizes; either one makes a tiled window
  float.

### M4 Workspaces

- **Cycling.** Mod-n goes to n; when the current workspace is n mod 10 it goes to the
  next one in that class, wrapping. Stateless: no timeout.
- **Move.** Mod-Shift-n moves the focused window with the same cycling and stays put.
- **Decided.** One workspace per window and one visible workspace: awesome's tag
  toggles (Mod-Ctrl-n, Mod-Ctrl-Shift-n) are dropped.
- **Also.** Previous workspace (Mod-Escape), next and previous (Mod-Left, Mod-Right),
  per-workspace layout and master factor.

### M5 Rules and hooks

- **Rules.** Match on class, instance, title and window type; set float, geometry,
  center, master, workspace. The Zed rule is a test case.
- **Hooks.** `on :manage`, `:focus`, `:unmanage`, `:workspace` with a read-only query
  API (windows on a workspace, their class), so the 2nd-browser rule ports as written.
- **Defaults.** Dialogs, transients, splash and utility windows float.

### M6 cgroup spawning

- **Mechanism.** systemd D-Bus `StartTransientUnit` (`zbus`) or `systemd-run`; research
  picks one. Units keep today's naming: scope in `app-<name>.slice`.
- **Mapping.** Window to process through `_NET_WM_PID` and `/proc/<pid>/cgroup`, and
  windows of apps not spawned by sugoi are still managed.
- **Actions.** Kill the whole unit of the focused window; resource limits per spawn
  from the config (memory, CPU weight).

### M7 EWMH and IPC

- **EWMH.** `_NET_SUPPORTED`, client list, active window, number and names of
  desktops, current desktop, window desktop, fullscreen and above states, struts for
  a bar.
- **IPC.** A Unix socket and `sugoictl` to query state, run actions and evaluate Ruby.
- **Bar.** The chosen bar (open question) shows workspaces and the systray in the
  integration suite or a manual check.

### M8 Release

- The daily-drive period, the footprint measurement, the parity check, the 0.1 report,
  the changelog, and release 0.1.0.

## Out of scope

- **Multi-monitor,** by decision: the user uses one monitor.
- **Workspace freezing,** by decision: it stops background work and breaks connections.
- **Bar, widgets, systray, notifications and menus,** left to external programs; the
  awesome status widgets move to the bar's scripts.
- **Fair, bottom and top tile layouts,** by decision: three layouts only.
- **Titlebars,** disabled in the current config too.
- **Compositing and Wayland;** picom can run beside sugoi.

## Open questions

- **Bar.** Which external bar replaces the wibar (taglist, tasklist, systray)? polybar
  and eww are not installed.
- **Notifications.** Which daemon replaces naughty? dunst is the usual X11 choice.
- **Banks.** 9 or 10 workspaces per bank (does Mod-0 count), and how many banks: 3?
- **Empty workspaces.** Does cycling skip empty workspaces past the first bank?
- **Daily drive.** How long a period closes 0.1? Judgment (agent): 5 working days.
- **Overlaps.** Mod-Shift-q and Mod-Ctrl-Return mean two things today, depending on
  focus. Keep that behavior or pick new chords?

## Risks

- **libruby drift.** Arch updates Ruby, and asdf shadows the system Ruby, so a build can
  link one Ruby and run another. M2 pins the libruby at build time and checks it at
  start; a pacman hook or static link is chosen in M2.
- **Hook latency.** A slow Ruby hook stalls window mapping. The query API is read-only
  and the watchdog bounds it; M5 measures map-to-placed time with and without hooks.
- **ICCCM corners.** Java, Electron, LibreOffice ("weird" in the current config) and
  Steam misbehave under minimal WMs. M1 to M3 test with the apps in the rules, and
  `parity.md` records what fails.
- **Losing the session.** A crash ends the X session. Until M8, sugoi runs in Xephyr or
  on a second TTY, and awesome stays the default session.
