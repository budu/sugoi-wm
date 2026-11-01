# Changelog

## Unreleased

- **Core window management** (M1). sugoi becomes the WM of an X11 display, adopts its
  windows, manages and unmanages them, focuses by map, pointer and click following the
  ICCCM input models, draws 3 px borders (`#aa007b` focused, `#000000` normal) and
  closes windows through `WM_DELETE_WINDOW` or `KillClient`. See `doc/spec/core.md`.
