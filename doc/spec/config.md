# Configuration

The source of truth for sugoi's Ruby configuration. Tests cite the identifiers in
brackets. Later milestones add to the DSL: layouts (M3), workspaces (M4), rules and
hooks (M5), spawn limits (M6).

## Loading

- **[L1] File.** sugoi loads `$XDG_CONFIG_HOME/sugoi/config.rb` (`~/.config/sugoi/` when
  unset), or the file given with `--config <path>`.
- **[L2] Ruby.** The file is plain Ruby, run by the CRuby sugoi is built against, with
  the DSL below available at the top level. Anything Ruby can do is allowed: `require`,
  helper methods, constants, reading other files.
- **[L3] Defaults.** Without a config file, or when the first load fails, sugoi runs on
  built-in defaults: the [F6] border, and the bindings `Mod4-Return` (spawn `xterm`),
  `Mod4-j` and `Mod4-k` (focus), `Mod4-Shift-q` (close), `Mod4-Shift-e` (quit),
  `Mod4-Ctrl-r` (reload), and `Mod4-1` to `Mod4-0` and `Mod4-Shift-1` to
  `Mod4-Shift-0` by keycode (`workspace` and `move_to` 1 to 10, [V4], [V8]).
- **[L4] Errors.** A load that raises, including a syntax error or an unknown key name,
  changes nothing: the previous config, or the defaults, stay live. The error is logged
  with its file and line and shown with `notify-send` (dunst).
- **[L5] Reload.** The `reload` action loads the file again. On success, the new
  bindings replace the old ones and the border settings apply to every managed window.
- **[L6] Load time limit.** A file still loading after 5 s is stopped, and the load
  fails ([L4]).
- **[L7] Taken chords.** A key chord another program has grabbed cannot be bound; sugoi
  names it with `notify-send` when the config loads, and the other chords work.

## DSL

- **[D1] Border.** `border width: 3, focused: "#aa007b", normal: "#000000"`; each
  keyword is optional and keeps its default.
- **[D2] Modkey.** `modkey "Mod4"` names the modifier that `Mod` stands for in chords;
  the default is `Mod4`.
- **[D3] Keys.** `key "Mod-Shift-j" do ... end` binds a chord to a block. A chord is
  modifiers and a key joined by `-`. Modifiers: `Mod`, `Shift`, `Ctrl` (or `Control`),
  `Alt` (`Mod1`), `Super` (`Mod4`), `Mod1` to `Mod5`. The key is an X keysym name as
  `xev` prints it (`Return`, `j`, `semicolon`, `XF86AudioMute`), or `#<keycode>` for a
  physical key (`#10` is the key labeled 1 on most layouts). A later `key` with the
  same chord replaces the earlier one.
- **[D4] Buttons.** `button "Mod-1" do ... end` binds a button chord (`1` to `5`) on
  managed windows: the window is focused, the press is consumed instead of reaching the
  window, and then the block runs. A chord without modifiers takes that button from
  every application, so window chords normally use `Mod`.
- **[D5] Actions.** Blocks call actions, which run in order once the block returns:
  - `focus :next` and `focus :previous` move the focus along the window order (the
    order windows were managed in, changed by `swap`), wrapping, to the next window that
    can take the focus ([F4]), and raise it.
  - `swap :next` and `swap :previous` swap the focused window with its neighbor in the
    window order; the focus stays on it. The cells stay: each window takes the other's
    place and its share of the column ([Y3]).
  - `close` closes the focused window ([K1]); `kill` ends its whole app ([A4]).
  - `spawn "kitty"` and `spawn "bash", "-ic", "~/bin/superkitty", name: "superkitty"`
    start a program ([P1]); `name` defaults to the basename of the first argument.
  - `sh "overmind restart -s ~/x.sock"` is `spawn "sh", "-c", ...` named `sh`.
  - `workspace 3`, `workspace :next`, `:previous` and `:back` switch workspaces
    ([V4] to [V6]); `move_to 3` moves the focused window ([V8]).
  - `reload` reloads the config ([L5]); `restart` replaces the sugoi process with a new
    one started the same way, which adopts the windows ([S2]); `quit` exits ([C2]).
- **[D6] Root buttons.** `root_button "3" do ... end` binds a button chord on the root
  window, where no window covers it, as awesome's `root.buttons` does.

## Running blocks

- **[R1] Errors.** A block that raises runs none of its actions; the error is logged and
  shown with `notify-send`, and sugoi carries on.
- **[R2] Time limit.** A block still running after 1 s is stopped as if it had raised
  ([R1]).
- **[R3] Keymap changes.** Key chords bind physical keys through the current keyboard
  mapping; when the mapping changes (`setxkbmap`), sugoi binds them again.
- **[R4] Lock keys.** Chords match whether Caps Lock and Num Lock are on or off.

## Spawning

- **[P1] Scope.** `spawn` runs `systemd-run --user --scope --collect --quiet
  --slice=app-<name>.slice -- <argv>`, as the awesome config does, so each app gets its
  own cgroup scope. It starts through `setsid -f`, so it inherits sugoi's environment
  but is not sugoi's child: no zombie is left when it exits, even after a restart, and
  no signal to sugoi's process group reaches it. Limits and killing an app's unit are in
  `doc/spec/apps.md`.
