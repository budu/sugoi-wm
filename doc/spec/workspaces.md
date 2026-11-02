# Workspaces

The source of truth for sugoi's workspaces on its one screen. Tests cite the
identifiers in brackets. Users number workspaces 1 to 30; EWMH numbers them 0 to 29.

## Model

- **[V1] Workspaces.** There are 30 workspaces in 3 banks of 10: 1 to 10, 11 to 20 and
  21 to 30. Exactly one is current; sugoi starts on workspace 1. Each managed window
  is on exactly one workspace, and a new window goes on the current one.
- **[V2] Shown windows.** A window is shown (mapped) when it is on the current
  workspace and not minimized; otherwise sugoi unmaps it and sets its `WM_STATE` to
  `IconicState`. Its geometry and states stay. Only shown windows take part in the
  layout, `focus`, `swap` and `restore` ([D5], [W6]).
- **[V3] Per workspace.** Each workspace has its own layout and master column ([Y7]);
  a load gives every workspace the first layout of the cycle, a reload keeps each one's
  layout if the cycle still has it, and gives all the configured master column.

## Switching

- **[V4] Cycling.** `workspace n` with n from 1 to 10 goes to the first workspace of
  n's class (n, n+10, n+20) after the current one, in that order and wrapping: from
  3, `workspace 3` goes to 13, then 23, then 3; from any workspace outside the class,
  to n. Empty workspaces are not skipped. `workspace n` with n from 11 to 30 goes to n.
- **[V5] Next and previous.** `workspace :next` and `workspace :previous` go to the
  workspace numbered one more or one less, wrapping from 30 to 1 and back.
- **[V6] Back.** `workspace :back` goes to the workspace that was current before the
  last switch; going to the current workspace is no switch.
- **[V7] Focus.** After a switch, the focus goes to the most recently focused shown
  window of the new workspace, or to the root without one. Crossings caused by the
  switch move no focus ([Z1]).

## Moving

- **[V8] Move.** `move_to n` puts the focused window on the workspace `workspace n`
  would go to from the window's own ([V4]), and the current workspace stays; the window
  is hidden, and the focus goes on as when a window closes ([Z2]).

## Across restarts

- **[V9] Kept workspaces.** sugoi sets each window's `_NET_WM_DESKTOP` and the root's
  `_NET_CURRENT_DESKTOP` (EWMH). On quit or restart every window is mapped again
  ([C2]); on start, a window with a valid `_NET_WM_DESKTOP` goes back on that
  workspace, and the root's `_NET_CURRENT_DESKTOP` names the current one. The rest of
  EWMH comes in M7.
- **[V10] Workspaces apps ask for.** A new window whose app set a valid
  `_NET_WM_DESKTOP` before mapping it, or asked for one with a `_NET_WM_DESKTOP`
  message before then ([E9]), goes on that workspace, unless a rule's `workspace:` puts
  it elsewhere ([U6]); otherwise, or for an invalid one (all workspaces is
  `0xFFFFFFFF`), on the current. Firefox restores its windows' workspaces so. On
  another workspace it stays hidden, and the focus stays where it was, as [U7] says.
- **[V11] Kept state.** sugoi keeps the rest of its state in the root's `_SUGOI_STATE`
  property, rewritten whenever it changes: the window order ([D5]) and the focus
  history ([F5]); which windows float, and every window's floating geometry ([W1]);
  the minimized windows, in the order they were minimized ([W6]); the stack shares
  ([Y3]); each workspace's layout and master width ([V3]), and the workspace before
  the last switch ([V6]). On start, after the config loads, the windows it adopts get
  that state back, over rules as their workspace does ([U6]); windows the state does
  not name come after those it does, in [S2]'s order. A saved layout the config's
  cycle lacks gives way to its first ([Y7]), and the master side stays the config's.
  The stacking order is X's, which adoption keeps ([S2]). So a restart, or a quit then
  a start, changes nothing on screen.
