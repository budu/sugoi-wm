# Core window management

The source of truth for how sugoi manages windows on one X11 screen. Tests cite the
identifiers in brackets. Later milestones extend this document: layouts (M3),
workspaces (M4), rules (M5), EWMH (M7).

sugoi is a non-reparenting window manager: it draws no frames or titlebars, so the
border is the client window's own X border, and windows stay usable if sugoi exits.

## Startup

- **[S1] One WM per display.** sugoi selects `SubstructureRedirect` on the root window.
  If another client holds it, sugoi exits with status 1 and the message "another
  window manager is running on <display>".
- **[S2] Adoption.** At startup, every child of the root that is viewable and not
  override-redirect is managed as if it had just been mapped, except that focus goes
  to the window under the pointer if there is one, else to the first shown window of
  the focus history. Adopted windows enter the focus
  history ([F5]) topmost first, unless sugoi's kept state orders them ([V11]), and
  keep their stacking order.
- **[S3] Connection loss.** If the X connection closes, sugoi exits with status 1 and
  logs the cause.
- **[S4] The root.** At startup the root window gets the arrow cursor (`left_ptr`)
  from the Xcursor theme and size the X resources name (`Xcursor.theme`,
  `Xcursor.size`, else from `Xft.dpi`), or X's cursor font without a theme; and a
  black background, which X repaints where no window covers it, unless a wallpaper is
  already set (`_XROOTPMAP_ID`, as `feh --bg-*` leaves). The wallpaper is the
  session's: `contrib/sugoi-session` runs `~/.fehbg`.

## Managing

- **[M1] Manage.** A `MapRequest` for a window that is not override-redirect makes it
  managed: sugoi sets its border width and normal border color, selects
  `EnterWindow`, `FocusChange`, `PropertyChange` and `StructureNotify` on it, sets
  `WM_STATE` to `NormalState`, maps it and focuses it ([F1]).
- **[M2] Override-redirect.** Override-redirect windows (menus, tooltips, popups) are
  never managed, focused or given a border.
- **[M3] Configure.** A `ConfigureRequest` from an unmanaged window is applied as
  asked. From a floating managed window, position and size are applied as asked;
  a tiled one keeps its cell ([Y9]). The border width and stacking stay sugoi's. When
  nothing changes, sugoi sends a synthetic `ConfigureNotify` with the current geometry
  (ICCCM 4.1.5).
- **[M4] Unmanage.** A managed window stops being managed when it is destroyed or
  unmapped by its client, also when that unmap reaches the server just before sugoi
  hides the window ([V2]); an app withdrawing a window sugoi has already hidden sends
  the synthetic `UnmapNotify` ICCCM 4.1.4 asks for. An unmapped window may
  be reused (a tray icon), so sugoi deletes its `WM_STATE` and `_NET_WM_DESKTOP`,
  releases its button grabs and event selection, and restores its original border
  width. If it had the focus, the next window gets it ([F5]).
- **[M5] Vanished windows.** Requests on a window that no longer exists fail with an
  X error; sugoi logs it at debug level and carries on.

## Focus

- **[F1] New windows take focus.** A newly managed window is raised and focused.
- **[F2] Sloppy focus.** The pointer entering a managed window focuses it. Only
  `EnterNotify` events in normal mode count; grab and ungrab crossings do not.
  Entering the window that already has the focus changes nothing, so a subwindow the
  client focused keeps it.
- **[F3] Click to focus.** A press of button 1, 2 or 3 on a managed window raises and
  focuses it, and the press still reaches the window. The wheel is not intercepted.
- **[F4] Input model** (ICCCM 4.1.7). Focusing a window sets the X input focus to it
  when its `WM_HINTS` input flag is true or absent, and sends `WM_TAKE_FOCUS` when its
  `WM_PROTOCOLS` lists it. A window with neither cannot be focused; the focus stays,
  but the window is still raised by [F1] and [F3]. `SetInputFocus` and the protocol
  messages use `CurrentTime` (decision notes, Core).
- **[F5] Focus history.** sugoi keeps the managed windows in focus order. When the
  focused window is unmanaged, the most recently focused remaining window gets the
  focus; with none left, the focus goes to the root (`PointerRoot`). If the unmap leaves
  the pointer over another managed window, that window gets the focus ([Z2]): the
  pointer wins over the history, as in awesome.
- **[F7] Malformed hints.** A `WM_HINTS` property sugoi cannot parse counts as absent.
- **[F6] Borders.** The focused window's border has the focused color, every other
  managed window's the normal color. Defaults, from the awesome config: width 3 px,
  focused `#aa007b`, normal `#000000`.

## Closing

- **[K1] Close.** Closing a window sends `WM_DELETE_WINDOW` when its `WM_PROTOCOLS`
  lists it, and otherwise kills its client (`KillClient`).

## Commands

Until the Ruby config (M2) and IPC (M7) exist, commands reach sugoi through its
library API (`Handle`), which the tests use.

- **[C1] Close focused.** Closes the focused window ([K1]); does nothing without one.
- **[C2] Quit.** sugoi exits with status 0. Managed windows stay mapped and get their
  original border width back, so a WM started next adopts them as they were.
- **[C3] Signals.** SIGTERM, SIGINT and SIGHUP quit as [C2] does, from before the
  settings load; the config cannot `trap` them.
