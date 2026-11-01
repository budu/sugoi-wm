# 0.1 progress

Milestone log for version 0.1 ([plan](plan.md)). The 0.1 report draws its effort,
correctness and footprint figures from here. Newest entries go at the top of each
section.

## Milestones

| Milestone | Started    | Closed     | Hours | Commits | Tests | Findings             |
|-----------|------------|------------|-------|---------|-------|----------------------|
| M2        | 2026-10-31 | 2026-10-31 | 0.5   | 7       | 68/68 | 17 fixed, 2 declined |
| M1        | 2026-10-31 | 2026-10-31 | 0.5   | 8       | 33/33 | 14 fixed, 3 declined |

Hours are active agent hours from `agent-hours` (the milestones skill's script); each
entry splits them into main thread and background. Findings count both review rounds;
a finding the user decides counts as fixed when a case pins the decision. Planning on
`plan-0.1` took 0.1 h. Commits count this repository's only.

## Footprint

Measured once, at the release (M8): RSS and idle CPU of the WM process. Reference:
awesome 4.3 used 68.8 MiB RSS and 0.6 % CPU on 2026-10-31, with its widgets.

## Entries

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
