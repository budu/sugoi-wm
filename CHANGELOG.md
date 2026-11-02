# Changelog

## Unreleased

- **The root** (trial). sugoi paints the root black unless a wallpaper is set, and
  gives it the arrow cursor from the Xcursor theme; drags show move and resize
  cursors. `contrib/sugoi-session` restores feh's wallpaper (`~/.fehbg`). See [S4] in
  `doc/spec/core.md`.
- **polybar** (trial). The bar scales its fonts with `Xft.dpi` and is 44 px high.

## 0.1.0 (2026-11-01)

The first release: an awesome replacement for one X11 monitor, configured in Ruby. See
`doc/plans/0.1/report.md`.

- **Known issues.** The first session as the user's WM found issues, not yet
  described; they open 0.2. Firefox's restored workspaces ([V10]) are not yet seen with
  Firefox itself.
- **Install** (M8). `scripts/install` builds `sugoi` and `sugoictl` into `~/.cargo/bin`
  and installs the session script, the GDM entry and the pacman hook.
- **Workspaces apps ask for** (M8). A new window goes on the workspace its app asks
  for through `_NET_WM_DESKTOP` (Firefox restoring its session), unless a rule says
  otherwise. See [V10] in `doc/spec/workspaces.md`.
- **EWMH and IPC** (M7). Bars and pagers see the windows, the active window, 30 named
  workspaces and the work area; docks reserve their edges; apps can go fullscreen
  (F11), pagers switch and activate. `sugoictl "<ruby>"` runs Ruby in sugoi through
  `$XDG_RUNTIME_DIR/sugoi/<display>.sock`. `contrib/polybar/` ports the awesome wibar,
  and `contrib/sugoi-session` starts dunst, polybar and sugoi. See
  `doc/spec/ewmh.md`.
- **Apps and cgroups** (M6). `spawn` takes `memory:` and `cpu_weight:` limits for the
  app's scope; `kill` ends every process of the focused window's app scope (never
  sugoi's own, nor the X session's), and closes the window otherwise; hooks see a
  window's `pid` and `unit`. A window's process comes from the X server, not
  `_NET_WM_PID`. See `doc/spec/apps.md`.
- **Rules and hooks** (M5). `rule class: "Thunar", float: true` and the like match
  windows by class, instance, title, type or transience, and float, size, center,
  raise, make master or send them to a workspace; dialogs, transients, splash and
  utility windows float by default. `on(:manage)`, `:focus`, `:unmanage` and
  `:workspace` run Ruby blocks with read-only windows. See `doc/spec/rules.md`.
- **Workspaces** (M4). 30 workspaces in 3 banks of 10: Mod-n goes to n, then n+10 and
  n+20 when pressed again; Mod-Shift-n moves the focused window the same way; next,
  previous and back; a layout per workspace. Workspaces survive a restart. See
  `doc/spec/workspaces.md`.
- **Layouts and window states** (M3). Tile (master on the right, 0.5 wide), max and
  float; master width and stack shares; floating, maximized, fullscreen, on-top and
  minimized windows; centering with an optional size; Mod-drag move and resize.
  Re-tiling never moves the focus. See `doc/spec/layouts.md`.
- **Ruby configuration** (M2). `~/.config/sugoi/config.rb` (or `--config`) sets the
  border, the modkey, key, button and root-button chords bound to Ruby blocks, and the
  actions focus, swap, close, spawn, sh, reload, restart and quit. Errors keep the
  current config and go to `notify-send`. Apps start in a systemd scope under
  `app-<name>.slice`. SIGTERM, SIGINT and SIGHUP quit cleanly. See
  `doc/spec/config.md` and the ported awesome config in `doc/config.rb`.
- **Core window management** (M1). sugoi becomes the WM of an X11 display, adopts its
  windows, manages and unmanages them, focuses by map, pointer and click following the
  ICCCM input models, draws 3 px borders (`#aa007b` focused, `#000000` normal) and
  closes windows through `WM_DELETE_WINDOW` or `KillClient`. See `doc/spec/core.md`.
