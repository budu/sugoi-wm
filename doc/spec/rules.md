# Rules and hooks

The source of truth for how sugoi's config treats windows by what they are. Tests cite
the identifiers in brackets.

## What a window is

- **[U1] Properties.** sugoi reads a window's class and instance (`WM_CLASS`), title
  (`_NET_WM_NAME`, else `WM_NAME`), type (the first of `_NET_WM_WINDOW_TYPE` it knows:
  `:normal`, `:dialog`, `:splash`, `:utility`, `:toolbar`, `:menu`; `:normal` without
  one) and whether it is transient (`WM_TRANSIENT_FOR`).

## Rules

- **[U2] Matching.** `rule class: "Thunar", float: true` matches windows whose
  properties equal every given matcher: `class:`, `instance:`, `title:` (a string, or an
  array of strings any of which may match), `type:` (a symbol or an array of them) and
  `transient:` (true or false).
- **[U3] Effects.** A matching rule sets any of: `float:` (true or false, [W1]),
  `size: [w, h]` (the window's size when it floats), `center: true` ([W7]),
  `on_top:` ([W5]), `master: true` (the window goes first in the window order, so it
  is the master, [Y2]) and `workspace: n` (1 to 30, exact; [V1]).
- **[U4] Order.** Every matching rule applies, in config order; a later rule's effect
  replaces an earlier one's.
- **[U5] Defaults.** Before the config's rules, transient windows and the types
  `:dialog`, `:splash` and `:utility` float ([W2] centers them); a config rule with
  `float: false` undoes it.
- **[U6] When.** Rules apply when sugoi manages a window, before it shows; for a window
  adopted at startup, a workspace kept in `_NET_WM_DESKTOP` ([V9]) wins over `workspace:`,
  and `size:` and `center:` do not apply: it keeps the geometry it had. For a new
  window, `workspace:` wins over the one its app asks for ([V10]).
  They apply again when a managed window's `WM_CLASS` changes, as some apps set it only
  after mapping.
- **[U7] Elsewhere.** A new window a rule puts on another workspace stays hidden, and
  the focus stays where it was ([F1] does not apply).

## Hooks

- **[H1] Events.** `on(:manage) { |window| ... }` runs after the rules, before the
  window shows, for new windows only (not those adopted at startup, so a restart does
  not run them again); `on(:focus)` after a window gets the focus; `on(:unmanage)`
  after a window is gone; `on(:workspace) { |n| ... }` after a switch, with the new
  number. These three run once sugoi has finished handling the event or the action that
  raised them, and see the state after it: after a key's single switch, the new
  workspace's window has the focus. Several blocks for one event run in config order;
  a `reload` among them ends that event's hooks.
- **[H2] Windows.** A hook's window is read-only: `id`, `class_name`, `instance`,
  `title`, `type`, `transient?`, `workspace` (1 to 30), `floating?` and `minimized?`,
  as they were when the hook started. Its methods `float`, `tile`, `size(w, h)`,
  `center`, `master`, `move_to(n)` and `close` act on it, like the actions of [D5];
  `size` and `center` float a tiled window, as the `center` action of [W7] does.
  `activate` goes to its workspace, restores it if minimized, and focuses and raises
  it, as a pager's activation does ([E8]).
- **[H3] Queries.** `windows` returns every managed window in the window order, as
  read-only windows; `windows(workspace: n)` only those on workspace n;
  `current_workspace` the current workspace (1 to 30). Hooks, key and button blocks
  and requests ([I2]) can query, each seeing the windows and the workspace as they
  were when it started.
- **[H4] Running.** A hook is a block: its actions run once it returns, errors and the
  1 s limit are as for keys ([R1], [R2]). Hooks do not run for what a hook's own
  actions cause.
