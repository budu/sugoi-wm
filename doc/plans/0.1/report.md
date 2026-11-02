# sugoi-wm 0.1 report

Results of version 0.1, which replaces awesome as the daily window manager on a
single-monitor X11 session: a Rust core, a Ruby configuration, apps in their own cgroup
scopes, three layouts and workspaces that cycle past 9 ([plan](plan.md)). Figures come
from the [progress log](progress.md), `ps` on the live session, `agent-hours` and git;
judgments are the implementing agent's and are marked as such.

| Item            | Value                                                          |
|-----------------|----------------------------------------------------------------|
| Measured commit | `a1d60dc` on `m8-release` (M1 to M8)                           |
| Baseline        | None: the first release. awesome 4.3 is the footprint reference |
| Benchmarks      | None planned; the footprint once, on the live session          |
| Host            | The author's Arch Linux workstation, Linux 7.2.11; Xvfb        |
| Toolchain       | Rust 1.98.1, x11rb 0.14, magnus 0.9, Ruby 3.4.10 (pacman)      |
| Agent setup     | Claude Opus 5.5 implements; `/code-review high` reviews        |
| Calendar time   | 2026-10-31 to 2026-11-01                                       |

## Summary

| Criterion      | Result                                                            |
|----------------|-------------------------------------------------------------------|
| Parity         | Met: 46 rows, 38 delivered in `doc/config.rb`, 8 dropped with why |
| Config safety  | Met: syntax error, raising and looping hooks keep the old config  |
| Tests          | Met: 204 of 204 pass, at scale 20 too                             |
| Stability      | Not met: the trial was cut short, by the user's decision          |
| Footprint      | 14.7 MiB RSS, 0.1 % CPU on the live session (awesome: 68.8 MiB)   |
| Reviews        | 133 findings over 14 rounds (6 P1, 51 P2, 76 P3); 120 fixed       |
| Effort         | 3.7 active hours (planning 0.1, M1 to M8 3.6)                     |
| Size           | 4,727 lines of Rust and Ruby, 4,354 lines of tests                |

The plan's scope is delivered and every measure but stability is met. sugoi runs as the
session WM from GDM; the user's first session found issues right away, and the user
chose to release 0.1 as it is and make those issues the evidence for 0.2, rather than
hold the release for the 2-day trial.

## What 0.1 delivers

- **M1 Core.** sugoi becomes the WM of a display, adopts and manages windows, focuses
  by map, pointer and click per ICCCM, draws borders and closes windows. Decided: the
  pointer wins over the focus history after a close (user).
- **M2 Ruby config.** Embedded CRuby: keys, buttons and root buttons bound to Ruby
  blocks, live reload that keeps the previous config on error, time limits on blocks
  and loads, apps spawned in systemd scopes. Decided: Mod-Shift-q closes, Mod-Shift-e
  quits, Mod-Ctrl-Return opens a session launcher (user).
- **M3 Layouts.** Tile (master on the right), max and float; floating, maximized,
  fullscreen, on-top and minimized windows; Mod-drag move and resize. Decided: no swap
  with master (user).
- **M4 Workspaces.** 30 workspaces in 3 banks of 10, Mod-n cycling n, n+10, n+20; move,
  next, previous and back; a layout per workspace; workspaces survive a restart.
  Decided: empty workspaces are not skipped, no tag toggles (user).
- **M5 Rules and hooks.** Rules by class, instance, title, type and transience; hooks
  on manage, focus, unmanage and workspace, with read-only windows; the awesome rules
  and the second-browser hook ported.
- **M6 cgroup spawning.** Memory and CPU limits per spawn; `kill` ends a window's app
  scope, never sugoi's own; a window's process from the X server.
- **M7 EWMH and IPC.** Bars and pagers see windows, workspaces and the work area; docks
  reserve edges; apps go fullscreen; `sugoictl` runs Ruby in sugoi. polybar replaces
  the wibar, dunst the notifications (user).
- **M8 Release.** A new window goes on the workspace its app asks for ([V10]), so
  Firefox can restore its windows' workspaces, as under awesome (user). A GDM session
  entry and `scripts/install`.

## Footprint

| WM            | RSS      | CPU                          | Source                        |
|---------------|---------:|------------------------------|-------------------------------|
| sugoi, live   | 14.7 MiB | 1 s over 15 min 41 s (0.1 %) | `ps`, 2 windows, 2026-11-01   |
| sugoi, Xvfb   | 15.2 MiB | about 0.05 % idle            | release build, 2026-11-01     |
| awesome 4.3   | 68.8 MiB | 0.6 % (917 s over 42.7 h)    | `ps`, with widgets, 2026-10-31 |

The baseline for 0.2 is the live row. Most of sugoi's RSS is libruby: the M2 spike
measured 13.9 MiB for Ruby alone. Judgment (agent): awesome's figure includes its
widgets, now in polybar's process, so the comparison flatters sugoi; a 15-minute
session with 2 windows is a thin sample, to remeasure over a full day in 0.2.

## Answers that changed

What an awesome user now sees differently:

- **Mod-Shift-q closes the focused window** (M2), and never quits; Mod-Shift-e quits,
  after a dmenu confirmation (M7).
- **One workspace per window, one visible** (M4): awesome's tag toggles are gone.
- **Mod-n cycles** (M4): pressing it on workspace n goes to n+10, then n+20.
- **The window list is gone** (M7): polybar shows workspaces, the title and the tray.
- **A window's process comes from the X server** (M6), not `_NET_WM_PID`.

## Correctness

| Milestone | Cases | Rounds | P1 | P2 | P3 | Findings |
|-----------|------:|-------:|---:|---:|---:|---------:|
| M1        |    33 |      2 |  2 |  8 |  7 |       17 |
| M2        |    68 |      2 |  1 |  7 | 11 |       19 |
| M3        |   114 |      2 |  1 |  7 | 12 |       20 |
| M4        |   139 |      2 |  0 |  4 | 15 |       19 |
| M5        |   160 |      2 |  0 |  6 | 14 |       20 |
| M6        |   171 |      2 |  2 | 10 |  7 |       19 |
| M7        |   200 |      2 |  0 |  9 | 10 |       19 |
| M8        |   204 |      0 |  0 |  0 |  0 |        0 |

- **Resolution.** 120 fixed, each P1 and P2 with a case that fails on the code before
  its fix; 13 declined with reasons in the progress log.
- **The P1s.** A stale `WM_TAKE_FOCUS` stamp and a malformed `WM_HINTS` that ended sugoi
  (M1); CRuby's signal handlers ignoring SIGTERM (M2); a master width above 1.0 that
  underflowed (M3); `kill` trusting `_NET_WM_PID` and a process that could exit under
  it, both able to kill an unrelated app (M6).
- **Property tests** (from M4) found real defects on their own: a tile rounding that
  pushed the last cell out (M4), a window sent elsewhere by a rule left `NormalState`
  (M5). The M8 extension fails on each of three mutations of [V10].
- **M8 had no review round:** its change ([V10]) is small and covered by 4 cases and
  the history property, and the user chose to release as is.

## Effort

| Milestone  | Hours | Main thread | Background | Commits |
|------------|------:|------------:|-----------:|--------:|
| Planning   |   0.1 |         0.1 |        0.0 |       — |
| M1         |   0.5 |         0.5 |        0.0 |       8 |
| M2         |   0.5 |         0.5 |        0.0 |       7 |
| M3         |   0.6 |         0.6 |        0.0 |       7 |
| M4         |   0.5 |         0.5 |        0.0 |       8 |
| M5         |   0.4 |         0.4 |        0.0 |       9 |
| M6         |   0.3 |         0.3 |        0.0 |       7 |
| M7         |   0.4 |         0.4 |        0.0 |       9 |
| M8         |   0.4 |         0.4 |        0.0 |       8 |
| Total      |   3.7 |         3.7 |        0.0 |      63 |

Hours are active agent hours from `agent-hours`; the user's time (reviews of the plan,
merges, the session) is not counted, nor the reviews' own runs, which ran inline.
Judgment (agent): no earlier version to compare with; the milestones were even, 0.3 to
0.6 h, with EWMH and IPC (M7) no costlier than core.

## Size

| Release or milestone | Code  | Added | Tests | Added |
|----------------------|------:|------:|------:|------:|
| 0.1 (M8)             | 4,727 |     7 | 4,354 |   124 |
| M7                   | 4,720 |   867 | 4,230 |   492 |
| M6                   | 3,853 |   248 | 3,738 |   173 |
| M5                   | 3,605 |   759 | 3,565 |   473 |
| M4                   | 2,846 |   319 | 3,092 |   989 |
| M3                   | 2,527 |   953 | 2,103 |   603 |
| M2                   | 1,574 |   972 | 1,500 |   637 |
| M1                   |   602 |   602 |   863 |   863 |

Code is Rust and Ruby under `crates/*/src` and `crates/sugoi/ruby`; tests are
`crates/*/tests`.

## Deferred

- **Out of 0.1's scope.** Multi-monitor, workspace freezing, widgets and menus in the
  WM, the window list, other tile layouts, swap with master, titlebars, compositing
  and Wayland (the plan).
- **The issues of the first session.** Found by the user on 2026-11-01, not yet
  described; they are 0.2's first evidence.
- **Firefox's restored workspaces** ([V10]): cased, but not yet seen with Firefox.
- **The `kill` and limit bindings**, to discuss with the user (M6, M7).
- **Hook latency** was not measured (the plan's M5 said it would be).
- **Left open.** c1's two early timeouts, never seen since (M1); the synthetic unmap
  branch no case can stage (M3); Ruby threads in a config starve while sugoi waits in
  `poll` (M2).

## Risks, in retrospect

- **libruby drift.** Not hit: sugoi links the pacman Ruby, pinned at build time, and
  `contrib/sugoi-ruby.hook` warns after a Ruby upgrade; no upgrade came during 0.1.
  Judgment (agent): the hook only warns; 0.2 could rebuild, or the session script
  could check `ldd` before starting sugoi.
- **Hook latency.** Not measured, contrary to the plan; the 1 s limit per block bounds
  the worst case. Judgment (agent): measure map-to-shown time in 0.2, with the user's
  hooks.
- **ICCCM corners.** Only partly checked: the cases use a test client, and the live
  classes were read in M5; Java, Electron, LibreOffice and Steam were not run under
  sugoi. Judgment (agent): the first session's issues may be here; each one becomes a
  case before its fix.
- **Losing the session.** Not hit: no panic and no core dump so far, and the session
  script falls back to awesome if sugoi cannot start. Judgment (agent): a crash
  mid-session still ends it; a restart-on-crash loop in the session script is cheap.
