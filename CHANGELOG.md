# Changelog

## Unreleased

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
