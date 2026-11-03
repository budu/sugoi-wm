# Implementation decisions

What the plans leave open, decided as the milestones go. Newest entries at the top of
each section.

## Core

- **The pointer wins over the focus history** (user, 2026-10-31). When closing a window
  leaves the pointer over another one, the crossing focuses it, as in awesome; ignoring
  those crossings would leave focus and pointer apart after most closes once layouts
  tile the screen.
- **`CurrentTime` for focus and protocol messages** (M1). With event timestamps, a
  focus set earlier at `CurrentTime` moved the server's last-focus-change time past later
  stamps, and X ignored the requests: f1 and f5 failed 10 times in 30 runs. The review
  found the same for `WM_TAKE_FOCUS`, which a globally active client echoes into its own
  `SetInputFocus`. A fresh server timestamp (a zero-length property change and its
  `PropertyNotify`) would satisfy ICCCM to the letter; dwm also sends `CurrentTime`.
- **Click grabs on buttons 1 to 3** (M1, review). A synchronous grab on every button sent
  each wheel tick through sugoi before the window got it; the wheel focuses nothing.
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

- **`root_button` apart from `button`** (M2, review). One `button` for both made a root
  menu chord take that button from every window, as awesome's `root.buttons` and
  `clientbuttons` are apart.
- **Signals before settings** (M2, review). `Wm::connect`, then `quit_on_signals`, then
  `Wm::start`: a SIGTERM during a slow load still quits cleanly. The prelude resets
  Ruby's handlers and refuses `trap` on TERM, INT and HUP.
- **5 s to load, 1 s per block** (M2, review). A config may `require` gems, so its load
  gets longer than a key press.
- **A pacman hook, not a static libruby** (M2). Arch has no `libruby-static.a`;
  `contrib/sugoi-ruby.hook` warns when a Ruby upgrade leaves sugoi without its libruby.
- **The pacman Ruby, dynamically linked** (M2). `.cargo/config.toml` sets `RUBY` to
  `/usr/bin/ruby` (3.4.10), since asdf's 4.0.1 comes first on `PATH` in a shell but
  lives in the home directory and may not be on `PATH` when X starts. Arch ships no
  `libruby-static.a`, so a Ruby major upgrade needs a rebuild of sugoi.
- **The binary owns Ruby** (M2). CRuby starts once per process, on one thread, so the
  library takes a `Script` and the binary passes `RubyScript`; the in-process tests use
  the built-in `Defaults`, and the config cases run the binary.
- **Ruby keeps the config objects** (M2). magnus 0.9 has no boxed GC root, so
  `Sugoi.load` keeps the candidate, `Sugoi.commit` makes it current once sugoi has
  checked its chords, and Rust holds only block indices.
- **Actions queue commands** (M2). A block's actions append to a list that sugoi runs
  after the block returns, so Ruby never calls into the WM, and a failed block runs
  none of them ([R1]).
- **`Timeout` for the 1 s limit** (M2). Measured at 7 µs per guarded call; it stops a
  pure-Ruby loop at the next 100 ms tick (0.3 s for a 0.2 s limit).
- **Key names from libxkbcommon** (M2). `xkb_keysym_from_name` knows every name `xev`
  prints; `xkeysym` has no lookup by name.
- **Syntax errors on one line** (M2). Ruby 3.4's Prism prints the source under the
  error; sugoi keeps the location and the messages for the notification.
- **CRuby, embedded** (user, 2026-10-31). Full Ruby and gems through `magnus`, rather
  than mruby: the config is personal, so sandboxing matters less than the language and
  the bindings. The cost is a build tied to one libruby (0.1 plan, Risks).
- **One meaning per chord** (user, 2026-10-31). awesome's overlaps were mistakes:
  Mod-Shift-q kills the focused window, Mod-Shift-e quits, Mod-Ctrl-Return opens
  a session launcher. Bindings never depend on whether a window has focus.

## Layouts

- **Tiled drags resize the layout** (0.2 trial, user, 2026-11-01). 0.1 floated a
  dragged tiled window, which the user found wrong: awesome's tile moves the pointer to
  the edge between the columns and sets the master width and the window's height
  factor, and its tiled move swaps. sugoi solves for the share that puts the window's
  edge at the pointer with the others in proportion (awesome's `setwfact`), at once
  rather than by awesome's feedback over motions.
- **Crossings from sugoi's own moves are dropped** (M3). After an arrange or restack that
  moved something, a round trip, then every queued `EnterNotify` is dropped and the
  other events are kept for the loop. Otherwise each re-tile would hand the focus to
  whatever slid under the resting pointer, as awesome and dwm also avoid.
- **The pointer wins after a close, explicitly** (M3). With crossings dropped, the
  decision of 2026-10-31 needs a `QueryPointer` after re-arranging ([Z2]).
- **Floating geometry is kept apart from the placed one** (M3). Each window has its
  floating rectangle and the rectangle sugoi last configured; maximize and fullscreen
  only change what is placed, so toggling back needs no saved geometry.
- **Geometry actions float a tiled window** (M3). `center`, `move` and `resize` take it
  out of the layout where it is; in awesome they did nothing visible on a tiled window.
- **sugoi's unmaps are counted** (M3). Only the root's `UnmapNotify` counts, so the
  window's own copy is not a second one; a synthetic one is a withdrawal (ICCCM 4.1.4).
  M4 hides workspaces the same way.
- **Size hints are ignored when tiling** (M3). Cells fill the column exactly, as the
  awesome config had no gaps; increments would leave holes.
- **Core cases run in the float layout** (M3), and test windows give a user position,
  so they stay where the cases put them.
- **Tile, max and float only** (user, 2026-10-31). awesome's fair, bottom and top
  variants are dropped.
- **No swap with master** (user, 2026-10-31). Swapping with the next or previous window
  stays; Mod-Ctrl-Return goes to a session launcher.

## Workspaces

- **sugoi's state lives on the root** (0.2 trial, user, 2026-11-01). The window order
  and the rest existed only in memory, so a restart rebuilt them from the stacking,
  reversing the stack as awesome does. `_SUGOI_STATE` holds them as text (readable with
  `xprop`), rewritten by difference before each wait and at exit, so it survives a
  crash too. The stacking is X's own and is not saved: adoption had pushed windows
  topmost first into a bottom-first list, which inverted floating windows. A stale
  window ID in the state could match a new window that reused it while no WM ran; the
  window would only get an order, a share or a floating flag.
- **One rule for mapping** (M4). A window shows when it is on the current workspace and
  not minimized; `show` maps or unmaps to match, counting its unmaps, so minimize,
  switches and moves cannot disagree on a window's state.
- **Hidden windows are iconic** (M4). Windows on other workspaces get `IconicState`, as
  minimized ones do, and keep their geometry and states.
- **The focus after a switch is the history's** (M4), as awesome restores a tag's last
  focused client; crossings from the switch are dropped. The pointer wins only after a
  close ([Z2]).
- **`_NET_WM_DESKTOP` now, the rest of EWMH in M7** (M4). A restart must keep each
  window's workspace, and that property is where EWMH keeps it; quit maps every window
  so the next WM adopts them.
- **`workspace 11` to `30` are exact** (M4). Only 1 to 10 cycle, so a config can name a
  later workspace directly; rules (M5) can use the same numbers.
- **The defaults bind the workspace keys** (M4), so a session without a config can
  still use more than one workspace.
- **Properties need a barrier** (M4). A configure request on an unmapped window of the
  test's own reaches sugoi after everything sent before, changes no managed state, and
  shows in the window's width once handled.
- **Cycling by repeat** (user, 2026-10-31). Mod-n goes to n, then n+10, n+20, wrapping,
  decided by the current workspace rather than a timeout.
- **Cycling skips empty stacked workspaces** (user, 2026-11-03). Stepping through
  empty ones cost presses in daily use, so a repeated Mod-n visits only n and the
  stacked workspaces that hold a window, minimized ones included; n stays a dependable
  home, even empty. A new stacked workspace is an explicit key (Mod-Ctrl-n, and
  Mod-Ctrl-Shift-n to move a window there), written in the config from `windows` and
  the exact `workspace 13`, not a core action: the core needed no new command, and
  its tests no notification. This replaces 0.1's "empty workspaces are not skipped".
- **3 banks of 10** (user, 2026-10-31). 30 workspaces; Mod-0 is workspace 10, so the
  cycle step is 10 and Mod-3 pressed twice reads as workspace 13.
- **One monitor** (user, 2026-10-31). No multi-monitor support.

## Interop

- **EWMH is published from the event loop** (M7). Before it waits, sugoi compares the
  client lists, the active window, the work area and each window's state with what it
  last wrote, and writes the differences: no path that changes state can forget.
- **Docks are mapped, never managed** (M7), and their struts make the work area;
  fullscreen still covers the whole screen. polybar maps its bar as a dock.
- **`_NET_CURRENT_DESKTOP` requests are exact** (M7): a bar's click on 3 goes to 3, even
  from 3, unlike `workspace 3`, which cycles.
- **Apps choose a new window's workspace** (M8, user, 2026-11-01). Firefox restores
  each window's workspace (its session's `workspaceID`) through `_NET_WM_DESKTOP`, as
  awesome honors; sugoi honors it set before mapping, and a message sent before then
  is written as that property. A rule's `workspace:` still wins, as the user's choice.
- **The wallpaper is the session's** (0.2 trial, user, 2026-11-01). sugoi only paints
  the root black, and keeps a wallpaper a setter announced in `_XROOTPMAP_ID`; feh
  sets it from the session script, as awesome's theme did (`arch.png`, scaled).
- **Cursors from the X resources** (0.2 trial). x11rb's cursor loader reads the theme
  and size from `Xcursor.*`, else sizes from `Xft.dpi`, as Xlib clients do; it falls
  back to X's cursor font, so a missing theme still replaces X's cross.
- **Widget popups are their own program** (0.2 trial, user, 2026-11-01). Notifications
  time out, stack and fill the history; `sugoi-popup` draws awesome's popup (x11rb,
  Pango and Cairo, as awesome draws) from the widgets' scripts, outside the WM. It
  holds the pointer and keyboard, so the click that closes it goes nowhere else. Ruby
  widgets (a bar of its own) stay a question for the 0.2 plan.
- **Requests are Ruby** (M7). One socket verb does all: actions, queries and anything
  the config can do, with the 1 s limit; the user is the socket's only client
  (0600), so it opens nothing the config does not already.
- **sugoictl is a library too** (M7): the socket path and the protocol live in it, sugoi
  uses them, and sugoictl itself links no libruby.
- **Confirmations are config, not sugoi** (M7): dmenu asks, `sugoictl quit` or `close`
  answers; a block could not wait on dmenu within its 1 s. Since the 0.2 trial (user,
  2026-11-02), rofi asks, in the sugoi theme: "No" first, no text entry.
- **The widgets keep their scripts** (M7): one wrapper turns their first line into a
  polybar label and the rest into a dunst notification on click.
- **polybar as the bar** (user, 2026-10-31). It covers workspaces, the title and the
  systray through EWMH; the window list is dropped, as the user does not use it.
- **dunst for notifications** (user, 2026-10-31). It replaces awesome's naughty, and
  sugoi reports config errors through it.

## Rules

- **Rules are data, hooks are Ruby** (M5). Matchers and effects go to Rust, so rules
  need no Ruby call per window and the built-in ones work without a config; only hooks
  call into Ruby, once per event, with snapshots of the windows.
- **Exact strings or lists, no regular expressions** (M5), as awesome's `rule` and
  `rule_any`; Rust and Ruby regular expressions differ, and no awesome rule needed one.
- **Rules apply again when `WM_CLASS` changes** (M5). The awesome Zed rule never
  matched; an app setting its class after the map is one explanation, and this covers
  it either way. The user no longer uses Zed (2026-11-01): its binding and rule are
  gone from `doc/config.rb`.
- **Manage hooks only for new windows** (M5), so a restart does not run them again on
  every adopted window (the browser hook would float windows that were tiled).
- **No hooks during a hook's actions** (M5): a `:focus` hook that moves the focus would
  otherwise run itself without end.
- **Hidden windows refuse the focus** (M5), in `focus()` itself: X ignores the request,
  but sugoi would have believed the window focused.

## Spawning

- **`systemd-run`, not a D-Bus client** (M6). The plan left the choice to research:
  `systemd-run --scope` took about 5 ms a spawn, becomes the app (same process ID),
  and takes limits with `-p`; a D-Bus client would add zbus and a fork-then-register
  handshake for no gain. Scopes keep their `run-p<pid>-i<n>.scope` names.
- **`kill` only stops app scopes, never sugoi's own** (M6). The live session runs the
  WM in `session-3.scope`, so a window of something the X session started shares it;
  and the test shell ran inside Emacs's app scope. `kill` stops a unit only when it is a
  scope in the user manager's `app.slice` and not sugoi's cgroup; else it closes.
- **A window's process comes from the X server** (M6, review). `_NET_WM_PID` can be
  stale and reused by another app, or name a process in a sandbox's namespace, and
  `kill` would then end an unrelated app. X-Resource's `QueryClientIds` gives the
  process behind the window's connection, read by the server from the socket; Xorg and
  Xvfb both have the extension.
- **`kill` is SIGKILL** (M6, review): `systemctl stop` waits up to `TimeoutStopSec`
  (90 s) on an app that ignores SIGTERM; awesome's kill was as abrupt (`KillClient`).
  `close` stays the gentle way.
- **Kill cases use real scopes they start** (M6), named `sugoi-test-*`, with an xterm
  inside, whose connection is the scope's process; test windows of the test process
  never get a `kill`. The own-scope guard is a unit test of a pure function, since
  testing it for real would aim at the editor running the tests.
- **Apps start through `setsid -f`** (M2, review). `systemd-run --scope` runs the app
  as the caller's child; through `setsid -f` it is never sugoi's, so nothing is left to
  reap after a restart (the first version reaped in a thread per app, and a restart
  left zombies). Ignoring `SIGCHLD` instead would break `Process.wait` in configs.
- **Scope names keep letters, digits and `_`** (M2). A `-` would nest the slice
  (`app-zed-editor.slice` under `app-zed.slice`).
- **No workspace freezing** (user, 2026-10-31). Freezing hidden apps' cgroups stops
  background work and breaks network connections.
