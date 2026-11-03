# Changelog

## Unreleased

- **Stacked workspaces in use** (trial). Repeating Mod-n cycles through n and the
  stacked workspaces that hold a window, skipping empty ones; Mod-Ctrl-n opens a new
  stacked workspace, and Mod-Ctrl-Shift-n moves the focused window to one. See [V4]
  in `doc/spec/workspaces.md` and `doc/config.rb`.
- **Memory in the bar** (trial). `contrib/polybar` shows the memory in use, as a
  percentage, beside the CPU.
- **Units in status modules** (trial). `contrib/polybar/status <icon> --unit % …` shows
  a percentage with its sign, colored with the number, as awesome's widgets did.
- **dunst** (trial). `contrib/dunst/` styles notifications as the rofi theme: dark,
  sugoi's magenta frame, rounded, under the bar, red for critical ones. Notifications
  and rofi take their icons from Papirus-Dark (`papirus-icon-theme`).
- **Ready to share** (trial). A README, CONTRIBUTING and an agent guide; an MIT
  license; a pinned toolchain; `scripts/gates`, `scripts/xephyr` (sugoi in a nested X
  server) and `scripts/check-deps`. `doc/config.rb`, polybar, rofi and the session
  script are generic examples now, without the author's own programs.
- **The root** (trial). sugoi paints the root black unless a wallpaper is set, and
  gives it the arrow cursor from the Xcursor theme; drags show move and resize
  cursors. `contrib/sugoi-session` restores feh's wallpaper (`~/.fehbg`). See [S4] in
  `doc/spec/core.md`.
- **polybar** (trial). The bar scales its fonts with `Xft.dpi` and is 44 px high.
- **Widget popups** (trial). Clicking a widget opens `sugoi-popup`, a styled popup
  under the pointer that the next click or key closes, instead of a notification; a
  popup can append several commands' output (`status --popup … --then …`). See
  `doc/spec/popup.md`.
- **rofi** (trial). `contrib/rofi/` themes the app launcher and dmenu lists: dark,
  sugoi's magenta accent, rounded, a larger font at 192 DPI, app icons.
- **Mouse drags as in awesome** (trial). In tile, Mod-right-drag on a tiled window
  resizes the layout (the master width and the window's edge) and Mod-drag swaps it
  with the window under the pointer; neither makes it float any more. A floating
  window resizes from its corner nearest the pointer. See [G1] to [G3] in
  `doc/spec/layouts.md`.
- **Restarts keep everything** (trial). sugoi keeps its window order, focus history,
  floating windows and their geometry, minimized windows, stack shares, and each
  workspace's layout and master width in the root's `_SUGOI_STATE`, and restores them
  on start: the stack no longer comes back reversed. Adopted windows keep their
  stacking, and the focus goes to the last focused window when the pointer is over
  none. See [V11] in `doc/spec/workspaces.md`.
- **A window list** (trial). Mod-w opens rofi's window mode: every window on every
  workspace, minimized ones included; picking one goes to it and restores it.
- **Minimized windows** (trial). Mod-Shift-w lists the current workspace's minimized
  windows in rofi; picking one restores it. Windows gain `minimized?` and `activate`;
  key and button blocks can query `windows`, as hooks do, and every block, hook and
  request knows `current_workspace` ([H2], [H3]).
- **Confirmations in rofi** (trial). Quit (Mod-Shift-e) and close (Mod-Shift-q) ask in
  a small rofi menu with the sugoi theme, "No" first, instead of dmenu.

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
