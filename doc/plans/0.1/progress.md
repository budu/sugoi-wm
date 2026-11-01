# 0.1 progress

Milestone log for version 0.1 ([plan](plan.md)). The 0.1 report draws its effort,
correctness and footprint figures from here. Newest entries go at the top of each
section.

## Milestones

| Milestone | Started    | Closed     | Hours | Commits | Tests   | Findings             |
|-----------|------------|------------|-------|---------|---------|----------------------|
| M7        | 2026-11-01 | 2026-11-01 | 0.4   | 9       | 200/200 | 17 fixed, 2 declined |
| M6        | 2026-11-01 | 2026-11-01 | 0.3   | 7       | 171/171 | 16 fixed, 3 declined |
| M5        | 2026-11-01 | 2026-11-01 | 0.4   | 9       | 160/160 | 19 fixed, 1 declined |
| M4        | 2026-11-01 | 2026-11-01 | 0.5   | 8       | 139/139 | 18 fixed, 1 declined |
| M3        | 2026-11-01 | 2026-11-01 | 0.6   | 7       | 114/114 | 19 fixed, 1 declined |
| M2        | 2026-10-31 | 2026-10-31 | 0.5   | 7       | 68/68   | 17 fixed, 2 declined |
| M1        | 2026-10-31 | 2026-10-31 | 0.5   | 8       | 33/33   | 14 fixed, 3 declined |

Hours are active agent hours from `agent-hours` (the milestones skill's script); each
entry splits them into main thread and background. Findings count both review rounds;
a finding the user decides counts as fixed when a case pins the decision. Planning on
`plan-0.1` took 0.1 h. Commits count this repository's only.

Property tests scale with `SUGOI_PROPTEST_SCALE`; `scripts/proptest-long` runs the
whole suite at 20 through nextest.

## Footprint

Measured once, at the release (M8): RSS and idle CPU of the WM process. Reference:
awesome 4.3 used 68.8 MiB RSS and 0.6 % CPU on 2026-10-31, with its widgets.

## Entries

### M7 EWMH and IPC

Status: closed 2026-11-01, on `m7-ewmh-ipc`, awaiting merge. No benchmarks: the
footprint is measured at the release. The manual polybar check is the user's: neither
polybar nor dunst is installed yet.

- **Research.** polybar's workspace module needs the desktop count, names and current
  desktop, the client list and each window's desktop; its title module the active
  window; its bar maps as a dock with struts. The awesome claude, codex and update
  widgets read scripts whose first line is "text severity" and the rest a popup.
- **Decisions.** EWMH published from the event loop by difference; docks mapped, not
  managed, and stacked under fullscreen only; exact desktop requests; activation
  across workspaces only from pagers and the user; requests are Ruby; sugoictl is a
  library too; confirmations are config; the widgets keep their scripts. All in the
  [decision notes](../notes.md#interop).
- **Changes.** `doc/spec/ewmh.md` with 14 behaviors; `wm/ewmh.rs`, `wm/ipc.rs`;
  `sugoictl` as library and binary; `contrib/polybar/` (config and a status wrapper),
  `contrib/sugoi-session`; `doc/config.rb` confirms quit and close through dmenu and
  `sugoictl`. Code: 4,720 lines (3,853 after M6); tests: 4,230 lines (3,738).
- **Tests.** 15 EWMH cases send client messages as bars and apps do; IPC cases in-process
  (socket, serving) and with the binary (Ruby requests, quit, the socket in spawned
  apps' environment, a looping `inspect` or message, an unread reply); sugoictl's exit
  codes against a fake server.
- **Review.** `/code-review high` on the branch, then on the fixes.

  | Round     | P1 | P2 | P3 | Resolution                          |
  |-----------|---:|---:|---:|-------------------------------------|
  | Review    |  0 |  6 |  4 | 10 fixed in `a6fe117`               |
  | Re-review |  0 |  3 |  6 | 7 fixed in `83464cf`, 2 declined    |

  The P2s: the bar could cover a fullscreen window (twice, the second time through a
  dock mapped later); a fullscreen state set before mapping was lost; user code in a
  reply (`inspect`, an error's message) ran past the limit; replies were written
  blocking; a huge strut overflowed; any app could switch the workspace, then, from my
  fix, none could restore its own minimized window. Each has a case that fails on the
  code before its fix. Declined: keeping `_NET_WM_STATE` on withdrawn windows (EWMH
  says remove it), and dropping the IPC client IDs.
- **Long runs.** `scripts/proptest-long`: 200 tests, 200 passed, in 1 min 43 s at
  scale 20, none leaky; no test scope or socket left.
- **Left open.** The polybar check by hand, once installed (`contrib/polybar/`); the
  `kill` and limit bindings, to discuss.
- **Hours.** 0.4 (main thread 0.4, background 0.0), from `agent-hours`.

### M6 cgroup spawning

Status: closed 2026-11-01, on `m6-cgroups`, awaiting merge. No benchmarks: spawning
is not a hot path, and the footprint is measured at the release.

- **Research.** `systemd-run --scope` took about 5 ms a spawn and becomes the app
  (same process ID), so it stays instead of a D-Bus client. The live session placed
  the WM in `session-3.scope`, apps in scopes under the user manager's `app.slice`
  (kitty gives each child its own), and the test shell inside Emacs's app scope:
  hence `kill`'s bounds and a guard for sugoi's own scope.
- **Decisions.** `systemd-run` kept; `kill` bounded to app scopes, never sugoi's;
  a window's process from the X server (X-Resource), checked by start time before a
  kill; `kill` is SIGKILL; kill cases on real scopes they start. All in the
  [decision notes](../notes.md#spawning).
- **Changes.** `doc/spec/apps.md` with 5 behaviors; `wm/apps.rs`; `spawn` takes
  `memory:` and `cpu_weight:`; the `kill` action, and `pid`, `unit` and `kill` on hook
  windows. Zed's binding and rule are gone from `doc/config.rb` (no longer used).
  Code: 3,853 lines (3,605 after M5); tests: 3,738 lines (3,565).
- **Tests.** Kill cases start their own scopes with an xterm inside, and run `kill` only
  on those; config cases cover limits, a bad and a zero limit, a hook seeing pid and
  unit, and the fallback to close when `systemctl` fails. Mutations skipping the kill,
  the `app.slice` check or the machine check (before X-Resource replaced it) each
  failed a case.
- **Review.** `/code-review high` on the branch, then on the fixes.

  | Round     | P1 | P2 | P3 | Resolution                          |
  |-----------|---:|---:|---:|-------------------------------------|
  | Review    |  1 |  6 |  3 | 9 fixed in `36ab195`, 1 declined    |
  | Re-review |  1 |  4 |  4 | 7 fixed in `f3f5fcc`, 2 declined    |

  The P1s: `kill` trusted `_NET_WM_PID`, which can be stale and reused by another app,
  and the X-Resource process can also exit while a child keeps the connection; both
  could have killed an unrelated app. The P2s: sandbox process IDs, late or oddly
  typed properties, `systemctl stop` waiting 90 s, `MemoryMax=0`, managing failing
  without X-Resource, and `setsid` hiding a failed kill. Declined: killing a terminal
  that shares an app's scope ([A4] says so), hooks' unit at manage time ([A5]), and an
  XID slot reused during manage.
- **Process.** One slip, caught at once: a `git checkout` used to undo a mutation
  reverted a file's uncommitted M6 changes, which were rewritten and committed before
  going on; mutations since restore from a copy.
- **Long runs.** `scripts/proptest-long`: 171 tests, 171 passed, in 1 min 43 s at
  scale 20, none leaky, no test scope left.
- **Left open.** EWMH (M7).
- **Hours.** 0.3 (main thread 0.3, background 0.0), from `agent-hours`.

### M5 Rules and hooks

Status: closed 2026-11-01, on `m5-rules`, awaiting merge. No benchmarks: the
footprint is measured at the release.

- **Research.** The live session's `WM_CLASS` values match the awesome rules' names
  (`Emacs`, `firefox`, `Chromium`, `kitty`). Zed's class could not be read: it needs
  DRI3, which Xvfb lacks, and its desktop file names none; an app setting its class
  after the map would explain the awesome rule's FIXME, so rules apply again on a
  `WM_CLASS` change.
- **Decisions.** Rules are data matched in Rust, hooks are Ruby blocks; exact strings
  or lists, no regular expressions; manage hooks only for new windows; no hooks during
  a hook's actions; focus, unmanage and workspace hooks run after their event or
  action; adopted windows keep their geometry; hidden windows refuse the focus. All in
  the [decision notes](../notes.md#rules).
- **Changes.** `doc/spec/rules.md` with 11 behaviors; `wm/rules.rs`; the DSL's `rule`,
  `on`, `Sugoi::Window` and `windows`; `doc/config.rb` ports the 8 custom awesome
  rules and the second-browser hook. Code: 3,605 lines (2,846 after M4); tests: 3,565
  lines (3,092).
- **Tests.** 11 rule cases in-process; hook cases run the binary (47 config cases in
  all). The history property gains maps that rules send to workspace 2 or float, and
  found a real defect on its first history, `[Map(Elsewhere)]`: such a window kept
  `NormalState` while hidden.
- **Mutations.** Of 13, 11 of the 12 that applied failed a case; the two gaps (an
  any-of matcher tried only on its first value, a hidden window focused in sugoi's
  state) got stronger cases.
- **Review.** `/code-review high` on the branch, then on the fixes.

  | Round     | P1 | P2 | P3 | Resolution                          |
  |-----------|---:|---:|---:|-------------------------------------|
  | Review    |  0 |  5 |  5 | 10 fixed in `0f02655`               |
  | Re-review |  0 |  1 |  9 | 9 fixed in `cbde9c8`, 1 kept        |

  The P2s: workspace and focus hooks ran before their event was done (acting on a
  hidden window, or with the pointer frozen by a click); a reload in a hook ran stale
  blocks; `window.center` and `size` did not float; restarts re-applied rule
  geometry; a reload dropped other events' queued hooks. Each has a case that fails on
  the code before its fix. Kept by decision: deferred hooks see the state after the
  whole action, now in [H1].
- **Long runs.** `scripts/proptest-long`: 160 tests, 160 passed, in 1 min 33 s at
  scale 20. The leaky test of M4 was the scope case's deliberately orphaned `sleep`;
  the case now ends it (`e0662cd`), and it is no longer leaky.
- **Left open.** Zed's actual `WM_CLASS` (the user's `xprop` can tell); EWMH (M7).
- **Hours.** 0.4 (main thread 0.4, background 0.0), from `agent-hours`.

### M4 Workspaces

Status: closed 2026-11-01, on `m4-workspaces`, awaiting merge. No benchmarks: the
footprint is measured at the release.

- **Research.** M3's counted unmaps already hid minimized windows; workspaces reuse
  them through one rule for what shows. A restart must keep each window's workspace,
  which is EWMH's `_NET_WM_DESKTOP`, so that property and the root's current desktop
  come now, the rest of EWMH in M7. Keycodes 10 to 19 are the keys 1 to 0 on Xvfb's
  and most layouts.
- **Decisions.** One mapping rule; hidden windows are iconic; the focus after a switch
  is the history's; `workspace 11` to `30` are exact; the defaults bind the workspace
  keys; sugoi's unmaps are told apart by sequence number. All in the
  [decision notes](../notes.md#workspaces).
- **Changes.** `doc/spec/workspaces.md` with 9 behaviors; `workspace.rs` and
  `wm/desks.rs`; per-workspace layouts; the DSL's `workspace` and `move_to`;
  `doc/config.rb` binds Mod-1 to Mod-0, their Shift chords, Mod-Left, Mod-Right,
  Mod-Escape and the root's wheel. Code: 2,846 lines (2,527 after M3); tests: 3,092
  lines (2,103).
- **Property tests.** New in M4 (`properties_layout`, `properties_workspaces`): tile
  cells checked pixel by pixel, share balance and growth, every workspace target
  against a table from the spec's words, and histories of up to 24 operations through
  X against a reference model, after a configure-request barrier. The tile property
  found a real defect at once: rounding each stack height could push the last cell
  past the column (a 38 px column, weights 0.5, 20, 1.5 and 0.01), fixed by rounding
  the cumulative boundaries.
- **Calibration.** 7 of 9 mutations of the workspace code fail a history (1 is
  equivalent; restoring from another workspace needs a 5-step story, caught in 5 of 6
  fresh gate runs); the M3 `grow_share` and a bank-blind cycling fail the pure
  properties. Widened after the review: withdrawals, plain and racing a switch under a
  server grab, which fail on the old counting rule in 4 of 4 fresh runs.
- **Review.** `/code-review high` on the branch, then on the fixes.

  | Round     | P1 | P2 | P3 | Resolution                          |
  |-----------|---:|---:|---:|-------------------------------------|
  | Review    |  0 |  4 |  6 | 10 fixed in `beaadbf`               |
  | Re-review |  0 |  0 |  9 | 8 fixed in `80bb68a`, 1 declined    |

  The P2s: an app's own unmap just before a switch hid its window was taken for
  sugoi's, so the window was never unmanaged; withdrawn windows kept
  `_NET_WM_DESKTOP`; the root had no current desktop until the first switch; a drag
  kept moving a window a switch had hidden. Each has a case that fails on `2e77069`.
  Declined: the 16-bit sequence window (over 32,768 requests before one event).
- **Long runs.** `scripts/proptest-long`: 139 tests, 139 passed, in 1 min 46 s at
  scale 20: 2,560 workspace histories, 30,720 cases of the pure properties. nextest
  marked one test leaky (a child process outlived it); not yet traced.
- **Left open.** The leaky test; EWMH desktop names, counts and client requests (M7).
- **Hours.** 0.5 (main thread 0.5, background 0.0), from `agent-hours`.

### M3 Layouts

Status: closed 2026-11-01, on `m3-layouts`, awaiting merge. No benchmarks: the
footprint is measured at the release.

- **Research.** The awesome theme has no gaps, and its steps are 0.05 for the master
  width and a window's height. Two findings shaped the design: under sloppy focus,
  every re-tile would hand the focus to whatever slid under the resting pointer, and
  minimizing needs sugoi's own unmaps told apart from withdrawals, as M4's hidden
  workspaces will. App fullscreen requests (F11) stay with EWMH in M7.
- **Decisions.** Crossings caused by sugoi's moves are dropped, and the pointer wins
  after a close by an explicit query; floating and placed geometry are kept apart;
  geometry actions float a tiled window; sugoi's unmaps are counted; size hints are
  ignored when tiling; stack shares are weights balanced to an average of 1.0. All in
  the [decision notes](../notes.md#layouts).
- **Changes.** `doc/spec/layouts.md` with 21 behaviors, core's [M3] and [F5] updated;
  `layout.rs` (pure geometry) and `wm/place.rs`; the DSL's `layouts`, `master`,
  `layout`, `master_width`, `window_height`, `toggle`, `minimize`, `restore`, `center`,
  `move` and `resize`; `doc/config.rb` ports the awesome layout, state and mouse
  bindings. Code: 2,527 lines (1,574 after M2); tests: 2,103 lines (1,500).
- **Tests.** 29 layout cases run in-process with fixed settings, `Handle` commands and
  XTEST drags; core cases moved to the float layout, and test windows give a user
  position. 33 core, 35 config, 29 layout cases and 17 unit tests.
- **Mutations.** Of 14, 12 failed a case at first; checks were added for the resize
  warp and a client withdrawing a minimized window. One still survives: the synthetic
  unmap branch, which matters only while one of sugoi's unmaps is pending, a race a
  case cannot stage.
- **Review.** `/code-review high` on the branch, then on the fixes.

  | Round     | P1 | P2 | P3 | Resolution                          |
  |-----------|---:|---:|---:|-------------------------------------|
  | Review    |  1 |  4 |  5 | 10 fixed in `0af7443`               |
  | Re-review |  0 |  3 |  7 | 9 fixed in `5c9058a`, 1 declined    |

  The P1: a config master width above 1.0 underflowed and ended sugoi. The P2s: drags
  took the focused window, not the clicked one, nor only the current press; a client
  re-mapping its minimized window stayed minimized; stack shares drifted, giving new
  windows arbitrary heights and others less than the floor; a no-op request from a
  floating window got no reply. Each has a case that fails on the previous code.
  Declined: removing the clamp in `tile()`, a cheap guard on that crash path.
- **Long runs.** Config passed 30 of 30 runs and the other suites 20 of 20, after the
  harness's `sync` began waiting for its marker to vanish (a full-screen marker made
  d6 fail once in 20).
- **Left open.** App fullscreen requests and `_NET_WM_STATE` wait for M7, as planned.
- **Hours.** 0.6 (main thread 0.6, background 0.0), from `agent-hours`.

### M2 Ruby config

Status: closed 2026-10-31, on `m2-ruby-config`, awaiting merge. No benchmarks: the
footprint is measured at the release; the Ruby spike's figures are below.

- **Research.** Both Rubies are built shared; sugoi links the pacman one (3.4.10)
  through `.cargo/config.toml`, since asdf's 4.0.1 lives in the home directory. A spike
  measured 24 ms to start Ruby, 13.9 MiB RSS, and `Timeout` stopping a runaway loop at
  0.3 s for a 0.2 s limit, at 7 µs per guarded call. CRuby starts once per process, on
  one thread, which shaped the tests. Arch ships no static libruby, so the libruby risk
  gets a pacman hook (`contrib/sugoi-ruby.hook`) instead.
- **Decisions.** The binary owns Ruby and the library takes a `Script`; Ruby keeps the
  config objects; actions queue commands; key names come from libxkbcommon; apps start
  through `setsid -f`; root chords are `root_button` ([D6]); signals are handled before
  the settings load. All in the [decision notes](../notes.md#configuration).
- **Changes.** `doc/spec/config.md` with 18 behaviors, and [C3] in the core spec; the
  DSL (border, modkey, key, button, root_button, 9 actions); `doc/config.rb`, the
  awesome bindings ported so far, loaded by a case; [`parity.md`](parity.md), where each
  of the 30 global keys, 9 client keys, tag keys, 6 mouse bindings and 9 rules lands.
  Code: 1,574 lines of Rust and Ruby (602 after M1); tests: 1,500 lines (863).
- **Tests.** 31 config cases run the binary under Xvfb with stub `systemd-run` and
  `notify-send` and XTEST key presses; 33 core cases and 4 unit tests pass unchanged.
- **Mutations.** Of 14 mutations, 12 fail a case, 1 did not apply (a script error), and
  1 showed a no-op: an `AllowEvents` after an asynchronous grab, removed in `7d84842`.
- **Review.** `/code-review high` on the branch, then on the fixes.

  | Round     | P1 | P2 | P3 | Resolution                                       |
  |-----------|---:|---:|---:|--------------------------------------------------|
  | Review    |  1 |  6 |  3 | 9 fixed and 1 split in `59f6067`                 |
  | Re-review |  0 |  1 |  8 | 8 fixed in `c4845ec`, 1 declined                 |

  The P1: CRuby's signal handlers made sugoi ignore SIGTERM. The P2s: `button("3")`
  took right-click from every app, a failed reload replaced a keyless config, focus
  stuck on an unfocusable window, apps became zombies after a restart, the load had
  no time limit, a taken chord failed silently, and a flaky case. Each P1 and P2 fix
  has a case that fails on the previous code. The split finding: the dead accessor is
  removed, caching the module lookup is declined (microseconds). Declined: a warning
  on window button chords without a modifier (the DSL is unreleased; [D4] says it).
- **Long runs.** Both suites passed 30 of 30 runs after the last fix.
- **Left open.** Confirmation prompts for quit and close wait for `sugoictl` (M7,
  `parity.md`). Autostart (`lxpolkit`) is not in the plan; the session script can
  start it. Ruby threads in a config starve while sugoi waits in `poll`.
- **Hours.** 0.5 (main thread 0.5, background 0.0), from `agent-hours`.

### M1 Core

Status: closed 2026-10-31, on `m1-core`, awaiting merge. No downstream clients and no
benchmarks: the footprint is measured once, at the release.

- **Research.** The awesome config sets 3 px borders, `#aa007b` focused and `#000000`
  normal; they are the defaults. `x11rb` 0.14 exposes its socket for `poll`, so a plain
  loop serves the X connection and a command queue without `calloop`. Xvfb's
  `-displayfd 1` gives each test its own server, and XTEST drives the pointer.
- **Decisions.** Non-reparenting; `CurrentTime` for `SetInputFocus` and protocol
  messages; click grabs on buttons 1 to 3 only; commands through a `Handle`; the pointer
  wins over the focus history (user). All in the [decision notes](../notes.md#core).
- **Changes.** `doc/spec/core.md` with 18 behaviors; the `sugoi` library and binary
  (602 lines of Rust with `sugoictl`'s stub); the Xvfb harness and 33 contract cases
  (863 lines), each citing its behavior.
- **Mutations.** 14 hand-made mutations of the core each fail at least one case. The
  first run found two blind spots (a missing raise, grab crossings), fixed in `1e18960`.
- **Bug found by repetition.** f1 and f5 failed 10 times in 30 runs: focus requests
  stamped with event times were ignored after a focus at `CurrentTime`. Fixed in
  `1e18960`.
- **Review.** `/code-review high` on the branch, then on the fixes.

  | Round     | P1 | P2 | P3 | Resolution                                      |
  |-----------|---:|---:|---:|-------------------------------------------------|
  | Review    |  1 |  6 |  2 | 9 fixed in `e33c2ea`, 1 user decision `1aa00c4` |
  | Re-review |  1 |  2 |  5 | 5 fixed in `1dc0a23`, 3 P3 declined             |

  The P1s: `WM_TAKE_FOCUS` carried a stale stamp, so globally active clients could not
  take the focus; a malformed `WM_HINTS` ended sugoi. Each P1 and P2 fix has a case
  that fails on the previous core. Declined: requests on a window unmapped just before
  its destroy (X's event order; errors log at debug), a fresh server timestamp instead
  of `CurrentTime` (recorded decision), and pipelining the remaining startup replies.
- **Smoke run.** The `sugoi` binary under Xvfb manages a real xterm (`WM_STATE` Normal,
  focused), and a second `sugoi` exits 1 with "another window manager is running".
- **Long runs.** The full suite passed 150 of 150 runs after the last fixes, and c1
  alone 400 of 400.
- **Left open.** c1 timed out twice in about 90 runs before the harness began logging
  (`1dc0a23`) and never since (0 in 550 suite and 400 single runs); the cause is not
  found. A manual run in Xephyr with the user's apps is the user's, before M8.
- **Hours.** 0.5 (main thread 0.5, background 0.0), from `agent-hours`.
