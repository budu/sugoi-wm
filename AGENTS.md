# sugoi agent guide

sugoi is an X11 tiling window manager in Rust, configured in embedded CRuby. Read
`README.md` first, then the specification document your change touches.
`CONTRIBUTING.md` sets the workflow, gates and conventions:

@CONTRIBUTING.md

## Documents

| File                     | Owns                                                       |
|--------------------------|------------------------------------------------------------|
| `doc/spec/*.md`          | Behaviors, by identifier; the source of truth              |
| `doc/config.rb`          | The example config; a test loads it                        |
| `doc/plans/notes.md`     | Live: implementation decisions the specification leaves    |
| `doc/plans/<version>/`   | Plans, progress logs, reports; closed versions are history |
| `CHANGELOG.md`           | User-visible changes, `Unreleased` first                   |
| `crates/`                | The implementation and its tests                           |
| `contrib/`, `scripts/`   | Session files, bar and menu configs; developer scripts     |
| `SEED.md`                | The seed prompt, historical: leave it unchanged            |

## Rules for agents

- **Branches.** Never commit to `main`, merge or push; work on a branch named after the
  change, and say when it is ready to merge.
- **The live session.** The developer's own sugoi may be running the shell you are in.
  Try changes under Xvfb or `scripts/xephyr`, never by restarting the developer's
  window manager unless asked. `sugoictl` talks to `$SUGOI_SOCKET` when it is set,
  which is the live sugoi; give a nested one its own socket path.
- **Processes.** Stop only processes you started, by PID. Never `pkill -f` or `pgrep -f`
  a pattern: it can match the developer's programs, and your own shell.
- **Undoing.** Restore a file from a copy you made, never with `git checkout` over work
  that is not committed.
- **Evidence.** Report what ran and its output. A test that was not run, or a step
  that was skipped, is said so.
