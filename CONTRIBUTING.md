# Contributing to sugoi

sugoi is its author's window manager, shared as a starting point. Forks are the
expected way to make it yours: change anything, the code is meant to be malleable.

Changes sent back are welcome when they are one of:

- **A clear fix:** a crash, a window lost or misplaced, a behavior that contradicts the
  specification in [`doc/spec/`](doc/spec/), with the steps that show it.
- **A very generic, simple feature:** something most users of any tiling window manager
  would want, small enough to read in one sitting, that keeps sugoi as it is (one
  monitor, X11, Ruby configuration, external bar and menus).

For anything else, open an issue first, or keep it in your fork. The
[README](README.md) has the developer walkthrough: prerequisites, commands and how to
try a change in a nested X server. Coding agents also follow [`AGENTS.md`](AGENTS.md).

## Workflow

1. Branch from `main`, named after the change (`fix-focus-after-close`).
2. Change the specification first, then the cases, then the code (below).
3. Commit in small steps; each commit passes the gates.
4. A maintainer reviews and merges. Do not push to `main`.

### Commits

- [Conventional Commits](https://www.conventionalcommits.org/): `feat`, `fix`, `docs`,
  `test`, `chore`, with the area as scope: `fix(layouts): …`, `feat(workspaces): …`,
  `docs(spec): …`. The subject says what changes; the body says why, with figures
  where there are some (cases added, a measurement).
- Stage files by path, not with `git add -A` or `git add .`.

## Quality gates

```sh
scripts/gates
```

It runs `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo nextest run --workspace`, stopping at the first failure. A commit of Markdown
only skips them; check its line widths and tables instead. Before a release, and after
changing a property test or what it covers, run `scripts/proptest-long` (property
cases times 20) and commit any `.proptest-regressions` file it writes.

## Changing behavior

The specification in `doc/spec/` is the source of truth. Each behavior has an
identifier (`[V11]`), and the tests cite it.

1. **Specification.** Change or add the behavior in its document, then every document
   that uses it, including [`doc/config.rb`](doc/config.rb) when an action changes.
2. **Cases.** One behavior per case, named after it (`v11_a_restart_keeps_the_order`),
   with a `/// [V11]` comment. A fix comes with a case that fails without it.
3. **Code.** Comments cite the identifier where code encodes a rule.
4. **Decisions** the specification leaves open go in
   [`doc/plans/notes.md`](doc/plans/notes.md), with the reason; user-visible changes
   go in the changelog's `Unreleased` section.

To check that a case can fail, break the change on purpose (one line), run the case,
and restore the file from a copy. A case that passes on broken code tests nothing.

## Changing the code

- **Rust owns the state.** Ruby configures and reacts; it never holds the window list.
  A block or hook runs within its time limit, and nothing in the event loop blocks
  on a client.
- **Never die on config.** A config error keeps the previous config and is reported.
- **Standards over widgets.** No bar, menu or notification inside sugoi: EWMH and the
  socket let other programs do that.
- **Tests stay in their sandbox.** Each test has its own Xvfb, and sugoi in-process or
  its binary on it. A test never touches the session it runs in: no `kill` or
  `pkill` by name, only processes it started; scopes it starts are named
  `sugoi-test-*`. Wait for a condition (`eventually`), not a fixed time.
- **Property tests** compare generated histories against a reference model written
  from the specification, not from the code. Case counts come from `cases()`, scaled
  by `SUGOI_PROPTEST_SCALE`.
- **Style.** `rustfmt` as configured; lines of code up to 100 characters. Comments say
  why. Add a dependency only when it earns its place, and note it in
  `doc/plans/notes.md`.

## Writing documents

- Markdown with hyphen bullets, a bold lead where it helps (`- **Drift.** …`), aligned
  tables and lines of at most 92 characters.
- ISO 8601 dates. Figures with their units and source; never invented.
- Refer to people as they/them unless their pronouns are known.

## Historical records

`doc/plans/<version>/` holds each version's plan, progress log and report, and
[`SEED.md`](SEED.md) the prompt that started the project. Closed versions are history:
leave them as they are, and put new results in new documents.
