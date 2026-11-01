# EWMH and IPC

The source of truth for what sugoi tells other programs, and takes from them: the EWMH
properties and messages bars, pagers and apps use, and sugoi's own socket. Tests cite
the identifiers in brackets.

## Properties

- **[E1] Identity.** sugoi creates a small unmapped window, sets `_NET_SUPPORTING_WM_CHECK`
  on it and on the root to point at it, and `_NET_WM_NAME` on it to `sugoi`; the root's
  `_NET_SUPPORTED` lists every hint of this page.
- **[E2] Windows.** The root's `_NET_CLIENT_LIST` lists the managed windows in the order
  they were managed, `_NET_CLIENT_LIST_STACKING` from bottom to top, and
  `_NET_ACTIVE_WINDOW` the focused one (`None` without one); they follow every change.
- **[E3] Workspaces.** `_NET_NUMBER_OF_DESKTOPS` is 30 and `_NET_DESKTOP_NAMES` names
  them `1` to `30`; `_NET_CURRENT_DESKTOP` and each window's `_NET_WM_DESKTOP` are [V9].
  `_NET_DESKTOP_GEOMETRY` is the screen's size and `_NET_DESKTOP_VIEWPORT` is `0, 0` for
  every workspace, as on any WM without large desktops.
- **[E4] Work area.** Windows of type `_NET_WM_WINDOW_TYPE_DOCK` (a bar) are mapped but
  not managed: no border, focus, layout or workspace. Their `_NET_WM_STRUT_PARTIAL`, or
  `_NET_WM_STRUT`, reserves screen edges; the work area ([Y1]) is the screen less what
  every dock reserves, and the root's `_NET_WORKAREA` gives it for each workspace. It
  follows docks mapping, unmapping and changing their struts.
- **[E5] Window states.** `_NET_WM_STATE` lists, for each managed window,
  `_NET_WM_STATE_FULLSCREEN`, `_NET_WM_STATE_ABOVE` (on top), `_NET_WM_STATE_HIDDEN`
  (minimized or on another workspace), and `_NET_WM_STATE_MAXIMIZED_VERT` and `_HORZ`
  (maximized), as [W3] to [W6] set them.
- **[E6] Allowed actions.** `_NET_WM_ALLOWED_ACTIONS` lists move, resize, fullscreen,
  maximize, minimize, close and change desktop for every managed window.

## Requests

- **[E7] State requests.** A client message `_NET_WM_STATE` (remove 0, add 1, toggle 2)
  for fullscreen, above or the two maximized states changes them as [W3] to [W5] do;
  an app's fullscreen (F11) works this way. Other states are ignored.
- **[E8] Activation.** `_NET_ACTIVE_WINDOW` for a managed window switches to its
  workspace, restores it if minimized, and focuses and raises it.
- **[E9] Desktops.** `_NET_CURRENT_DESKTOP` with n switches to workspace n + 1, as the
  numbers 11 to 30 do ([V4]), including from a bar's clicks and scrolls;
  `_NET_WM_DESKTOP` moves a window there.
- **[E10] Closing.** `_NET_CLOSE_WINDOW` closes the window as [K1] does.

## IPC

- **[I1] Socket.** sugoi listens on `$XDG_RUNTIME_DIR/sugoi/<display>.sock` (the display
  without its `:`), only its user may connect, and `$SUGOI_SOCKET` tells `sugoictl`
  and programs sugoi spawns where it is.
- **[I2] Requests.** A request is Ruby, run like a key's block ([R1], [R2]) with the
  actions of [D5] and the queries of [H3]: `sugoictl quit`, `sugoictl "workspace 3"`,
  `sugoictl "windows.map(&:title)"`. The reply is the block's value, as `inspect` gives
  it, once its actions have run; an error is replied as such.
- **[I3] sugoictl.** `sugoictl <ruby>` sends the request, prints the reply, and exits 0,
  or prints the error and exits 1; without a reachable sugoi it exits 2.
- **[I4] Many clients.** Requests from several clients are served one at a time, in
  the event loop; a client that sends nothing or reads nothing for 1 s is dropped,
  and never stops sugoi.
