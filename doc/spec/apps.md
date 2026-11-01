# Apps and their cgroups

The source of truth for how sugoi relates windows to the processes and systemd units
behind them. Tests cite the identifiers in brackets. Spawning itself is [P1].

- **[A1] Limits.** `spawn "firefox", memory: "4G", cpu_weight: 50` gives the app's
  scope `MemoryMax=4G` and `CPUWeight=50` (`systemd-run -p`). `memory:` is a byte count
  or a number with `K`, `M`, `G` or `T`; `cpu_weight:` is 1 to 10000. Anything else
  fails the block ([R1]).
- **[A2] A window's process.** A window's process is its `_NET_WM_PID`, when its
  `WM_CLIENT_MACHINE` is this host's name (ICCCM: a process ID means nothing on
  another machine). Windows without one are managed alike, without a process.
- **[A3] A window's unit.** A process's unit is the last part of its cgroup path, as
  `/proc/<pid>/cgroup` gives it (`run-p5151-i81971.scope`). Windows of apps sugoi did
  not spawn have whatever unit their process is in: an app started from a terminal
  may share the terminal's scope, or have its own (kitty gives each child one).
- **[A4] Kill.** `kill` stops the focused window's unit (`systemctl --user stop`), so
  every process of the app ends, when that unit is a scope inside the user manager's
  `app.slice` and not the scope sugoi itself runs in. Otherwise it closes the window
  ([K1]): a window in the X session's own scope, or sugoi's, must not end the session.
- **[A5] Hooks.** A hook's window also has `pid` and `unit` (nil when unknown), and a
  `kill` method ([H2]).
