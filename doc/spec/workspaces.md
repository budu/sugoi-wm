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
