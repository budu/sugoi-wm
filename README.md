# sugoi

A tiling window manager for X11, written in [Rust] and configured in [Ruby], as a
replacement for [awesome].

> **A personal window manager.** sugoi is built to work for its author, on one machine
> (Arch Linux, one monitor), and to be a good starting point for others. It is not meant
> to work for everyone out of the box: fork it and make it yours. Contributions are
> welcome only for clear fixes or very generic, simple features; see
> [CONTRIBUTING](CONTRIBUTING.md).

**Status:** the author's daily window manager. The [changelog](CHANGELOG.md) lists
releases and what changed.

## What it does

- **Tile, max and float.** The master column on the right (awesome's `tile.left`), the
  others stacked beside it. Mouse drags resize the layout or swap windows, as awesome's
  do; floating windows move and resize from any corner.
- **30 workspaces in 3 banks.** Mod-1 to Mod-0 go to workspaces 1 to 10; pressing the
  same key again cycles on to n+10 and n+20.
- **A Ruby configuration.** Keys, buttons, rules and hooks in
  `~/.config/sugoi/config.rb`, run by an embedded CRuby. A broken config never breaks
  the session: sugoi keeps the last one that loaded and says why.
- **Apps in their own cgroups.** Every program sugoi starts runs in its own [systemd]
  scope, with optional memory and CPU limits; `kill` ends a whole app.
- **Standards for the rest.** [EWMH] for bars and pagers ([polybar], [rofi]), and a socket
  that runs Ruby inside sugoi: `sugoictl "windows.map(&:title)"`.
- **Restarts change nothing.** Window order, floating windows, minimized ones and each
  workspace's layout survive a restart or a crash.

It does not do multiple monitors, Wayland, compositing, titlebars or widgets; external
programs draw the bar, menus and notifications. `contrib/` has the author's setup:
polybar, rofi, a session script and a small popup for bar widgets.

## Install

Requirements: Linux with X11 and a systemd user session, Rust (the toolchain in
`rust-toolchain.toml`), and Ruby built with a shared `libruby`. sugoi links the Ruby at
`/usr/bin/ruby` (`.cargo/config.toml`); change it there to use another.
`scripts/check-deps` says what is missing.

```sh
scripts/install              # sugoi, sugoictl and sugoi-popup into ~/.cargo/bin,
                             # then, with sudo, the session for GDM and a pacman hook
scripts/install --no-system  # the binaries only
cp doc/config.rb ~/.config/sugoi/config.rb
```

Pick "sugoi" at the login screen, or run `contrib/sugoi-session` from `~/.xinitrc`. The
session script restores the [feh] wallpaper, starts [dunst], polybar and sugoi, and
falls back to another window manager if sugoi cannot start. To use the bar and the
menus as they are, copy `contrib/polybar` and `contrib/rofi` to `~/.config/`.

## Configure

[`doc/config.rb`](doc/config.rb) binds every action sugoi has, with comments; the
programs it starts ([kitty], [Firefox], rofi) are examples. A small
config:

```ruby
modkey "Mod4"
layouts :tile, :max, :float

key("Mod-Return") { spawn "kitty" }
key("Mod-j") { focus :previous }
key("Mod-k") { focus :next }
(1..10).each { |n| key("Mod-##{9 + n}") { workspace n } }
button("Mod-1") { move }

rule class: "feh", float: true, center: true
on(:manage) { |window| window.master if window.class_name == "Emacs" }
```

Mod-Ctrl-r (in the example) reloads it. `sugoictl` runs Ruby in the running sugoi:
`sugoictl reload`, `sugoictl "workspace 3"`, `sugoictl restart` after installing a new
build.

## Documentation

The specification says what sugoi does, one numbered behavior at a time; the tests cite
those numbers.

| Document                                | Contents                                    |
|-----------------------------------------|---------------------------------------------|
| [Core](doc/spec/core.md)                | Startup, managing windows, focus, closing   |
| [Configuration](doc/spec/config.md)     | The config file, bindings, actions, reload  |
| [Layouts](doc/spec/layouts.md)          | Tile, max, float, states, stacking, drags   |
| [Workspaces](doc/spec/workspaces.md)    | Banks, cycling, moving, kept state          |
| [Rules and hooks](doc/spec/rules.md)    | Rules, hooks, the windows Ruby sees         |
| [Apps](doc/spec/apps.md)                | Limits, a window's process and unit, `kill` |
| [EWMH and IPC](doc/spec/ewmh.md)        | What bars see, the socket, sugoictl         |
| [Popups](doc/spec/popup.md)             | `sugoi-popup`, for bar widgets              |
| [Notes](doc/plans/notes.md)             | Decisions the specification leaves open     |
| [0.1 plan](doc/plans/0.1/plan.md)       | How sugoi was planned, by milestone         |
| [0.1 report](doc/plans/0.1/report.md)   | What 0.1 delivered: tests, reviews, effort  |
| [0.2 trial log](doc/plans/0.2/trial.md) | Fixes from the first days of daily use      |

## Developer walkthrough

### Prerequisites

| Tool                         | Used for                                              |
|------------------------------|-------------------------------------------------------|
| [`rustup`]                   | The pinned toolchain in `rust-toolchain.toml`         |
| [`cargo-nextest`]            | Tests: `cargo install cargo-nextest --locked`         |
| Ruby with a shared `libruby` | The embedded configuration (`.cargo/config.toml`)     |
| pango, cairo, libxkbcommon   | `sugoi-popup`'s text, key names                       |
| [`Xvfb`], [`xterm`]          | Every test runs its own X server; `kill` cases xterms |
| A systemd user session       | Spawning into scopes, and the `kill` cases            |
| [`Xephyr`] (optional)        | `scripts/xephyr`, to try sugoi in a window            |

`scripts/check-deps` checks them all.

### Layout

| Path                   | Contents                                                     |
|------------------------|--------------------------------------------------------------|
| `crates/sugoi`         | The window manager: library, binary, Ruby prelude and tests  |
| `crates/sugoictl`      | The socket's protocol, and the `sugoictl` command            |
| `crates/sugoi-popup`   | The popup bar widgets open                                   |
| `doc/spec/`            | The specification                                            |
| `doc/config.rb`        | The example config, which a test loads                       |
| `doc/plans/`           | Plans, logs and reports, and the implementation notes        |
| `contrib/`             | The session script, GDM entry, polybar, rofi, a pacman hook  |
| `scripts/`             | Developer scripts                                            |

### Build and test

```sh
cargo build                                     # every crate, debug
scripts/gates                                   # fmt, clippy, then the whole suite
cargo nextest run --workspace                   # the suite alone
cargo nextest run -p sugoi --test workspaces    # one file of cases
cargo nextest run --workspace -E 'test(/v11/)'  # cases whose name matches
scripts/proptest-long                           # property cases times 20
```

Each test starts its own Xvfb, runs sugoi in-process (or its binary, for config cases)
and drives it as X clients would: windows, client messages, XTEST keys and pointer.
Cases are named after the behavior they check (`v11_a_restart_keeps_the_window_order`
checks [V11] in `doc/spec/workspaces.md`). Property tests compare histories of
operations against a reference model; `SUGOI_PROPTEST_SCALE` multiplies their cases.

### Try it

```sh
scripts/xephyr                 # this checkout's sugoi in a window, with doc/config.rb
scripts/xephyr my-config.rb    # with another config
```

Apps it starts open inside the window. `SUGOI_XEPHYR_SCREEN=1920x1080` and
`SUGOI_XEPHYR_DPI=192` set its size and DPI; the script prints the `SUGOI_SOCKET` to
use with `sugoictl`. To replace the running window manager instead:
`scripts/install --no-system && sugoictl restart`. Logs go to the journal
(`journalctl --user -b`), with `RUST_LOG` as the filter.

## License

[MIT](LICENSE).

[awesome]: https://awesomewm.org
[`cargo-nextest`]: https://nexte.st
[dunst]: https://dunst-project.org
[EWMH]: https://specifications.freedesktop.org/wm-spec/latest/
[feh]: https://feh.finalrewind.org
[Firefox]: https://www.firefox.com
[kitty]: https://sw.kovidgoyal.net/kitty/
[polybar]: https://github.com/polybar/polybar
[rofi]: https://github.com/davatorium/rofi
[Ruby]: https://www.ruby-lang.org
[Rust]: https://www.rust-lang.org
[`rustup`]: https://rustup.rs
[systemd]: https://systemd.io
[`Xephyr`]: https://www.freedesktop.org/wiki/Software/Xephyr/
[`xterm`]: https://invisible-island.net/xterm/
[`Xvfb`]: https://www.x.org/releases/current/doc/man/man1/Xvfb.1.xhtml
