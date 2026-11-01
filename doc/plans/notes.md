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

- **Apps start through `setsid -f`** (M2, review). `systemd-run --scope` runs the app
  as the caller's child; through `setsid -f` it is never sugoi's, so nothing is left to
  reap after a restart (the first version reaped in a thread per app, and a restart
  left zombies). Ignoring `SIGCHLD` instead would break `Process.wait` in configs.
- **Scope names keep letters, digits and `_`** (M2). A `-` would nest the slice
  (`app-zed-editor.slice` under `app-zed.slice`).
- **No workspace freezing** (user, 2026-10-31). Freezing hidden apps' cgroups stops
  background work and breaks network connections.
