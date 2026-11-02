# 0.2 trial log

The user runs sugoi as the session WM from 2026-11-01, the day 0.1.0 was released, and
reports issues as they come. By the user's decision, 0.2 stays open during this trial:
each issue is fixed on its own branch, merged by the user, and recorded here; the 0.2
plan is drafted from this log once the trial ends. Figures come from git and
`agent-hours`; judgments are the implementing agent's and are marked as such.

## Fixes

Newest first. Hours are active agent hours from `agent-hours` (main thread only: no
background work ran); work on `main` before a branch counts for it.

| Issue, in short                  | Branch           | Spec         | Commits | Hours |
|----------------------------------|------------------|--------------|--------:|------:|
| This workspace's minimized only  | `minimized-list` | [H2], [H3]   |       3 |   0.1 |
| A way back to minimized windows  | `window-list`    | (config)     |       1 |   0.0 |
| A restart reverses the stack     | `kept-state`     | [V11], [S2]  |       2 |   0.2 |
| Drags float tiled windows        | `tiled-drags`    | [G1] to [G3] |       2 |   0.1 |
| drun, superkitty: bigger, darker | `rofi-theme`     | (contrib)    |       1 |   0.0 |
| Widget details open as alerts    | `popup-helper`   | [O1] to [O4] |       3 |   0.3 |
| No background, cross, tiny bar   | `trial-fixes`    | [S4], [G1]   |       3 |   0.2 |

Total: 15 commits, 0.9 h. Code grew from 4,727 to 5,476 lines of Rust and Ruby, tests
from 4,354 to 5,150 lines; the suite from 204 to 240 tests, all passing. The long
property run passed 237 of 237 at scale 20 in 1 min 50 s on `kept-state`.

- **Background, cursor, bar** (`trial-fixes`). sugoi painted no root and set no cursor,
  so the screen kept stale pixels and X's cross; it now paints the root black unless a
  wallpaper is set (`_XROOTPMAP_ID`), and sets the arrow, move and corner cursors from
  the Xcursor theme. The session script replays `~/.fehbg` (awesome's `arch.png`).
  polybar sized itself for 96 DPI on a 192 DPI screen; it now reads `Xft.dpi`.
- **Widget popups** (`popup-helper`). Notifications were the wrong medium (they time
  out and stack); `sugoi-popup`, a small Rust program (x11rb, Pango and Cairo),
  shows awesome's styled popup under the pointer until the next click or key. The
  claude popup lists the Claude Code sessions again, lost in the 0.1 port.
- **rofi theme** (`rofi-theme`). rofi ran with its default theme at 96 DPI. A dark
  theme with sugoi's magenta, in `contrib/rofi/` and linked from `~/.config/rofi`;
  superkitty's table uses a monospace font (a line in the user's dotfiles).
- **Tiled drags** (`tiled-drags`). 0.1 floated a dragged tiled window. As awesome's
  tile does, a resize now sets the master width and the window's edge, and a move
  swaps with the window under the pointer; floating windows resize from the nearest
  corner.
- **Kept state** (`kept-state`). The window order lived only in memory, so a restart
  rebuilt it from the stacking and reversed the stack, as awesome does. sugoi now
  keeps its state in the root's `_SUGOI_STATE` and restores it at start.
- **Window lists** (`window-list`, `minimized-list`). Mod-w opens rofi's window mode
  (every window; picking one activates it through EWMH); Mod-Shift-w lists the
  current workspace's minimized windows, built in the config's Ruby.

## Findings

- **GDM hides an entry it cannot check.** GDM evaluates `TryExec` as its own user, who
  cannot read a 0700 home, so the session script moved to `/usr/local/bin`
  (`scripts/install` installs it).
- **rofi 2 reads no X resources.** Its DPI must be set in the config (`dpi: 192`);
  polybar can read it (`${xrdb:Xft.dpi:96}`).
- **Restarts lost more than the order.** Floating windows came back tiled, minimized
  ones restored, layouts and master widths reset; and adoption inverted the stacking
  of floating windows (topmost first into a bottom-first list). All fixed by [V11]
  and [S2].
- **A crash still loses hidden windows.** Windows sugoi unmapped (other workspaces,
  minimized) are not viewable after a crash, so the next sugoi does not adopt them;
  the kept state names them, which could fix it. Not done.
- **Key blocks could not query windows.** [H3] gave `windows` to hooks and requests
  only; key and button blocks now see the same view, and `current_workspace`.
- **Firefox's restored workspaces** ([V10], M8) are not yet confirmed with Firefox.
- **Load makes timing cases flaky.** One `cargo test` run failed three timing cases
  while builds ran beside it; six interleaved runs of `main` and the branch had none.

## Process

- **No review round** ran on the trial fixes; each has contract cases, and mutations
  that fail them, but no `/code-review`. Judgment (agent): one review of the trial's
  diff (`v0.1.0..main`) before the 0.2 plan closes the gap cheaply.
- **Two slips, both caught.** A test cleanup (`pkill -f "rofi -dmenu"`) closed the
  user's open superkitty menu; cleanups now stop their own processes by PID. The
  `kept-state` commits first landed on `main`; they moved to their branch before any
  merge, and nothing was pushed.

## Open for the 0.2 plan

- **Ruby widgets.** A bar of sugoi's own, in Ruby (ruby-gnome), rather than polybar
  scripts; `sugoi-popup` would be its first piece (user, to decide).
- **`kill` and spawn limits** have no bindings yet (user, to discuss).
- **Adopting hidden windows after a crash**, from the kept state (finding above).
- **Hook latency**, never measured (0.1 report).
- **The footprint over a full day**, for 0.2's baseline (0.1 measured 15 minutes).
