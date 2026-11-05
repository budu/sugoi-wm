# Layouts and window states

The source of truth for how sugoi places windows on its one screen. Tests cite the
identifiers in brackets. M4 makes the layout, master width and window list per
workspace; until then there is one of each.

## Layouts

- **[Y1] Layouts.** A layout arranges the windows that are not floating, maximized,
  fullscreen or minimized (the tiled windows), in the window order ([D5]), inside the
  work area: the screen, until EWMH struts exist (M7). There are no gaps. sugoi
  arranges again whenever a window is managed, unmanaged, changes state, or the
  layout, master width, a window's share or the order changes.
- **[Y2] Tile.** The first tiled window is the master, alone in the master column; the
  others share the stack column, top to bottom. The master column takes the master
  width (default 0.5) of the work area, on the master side (default right, as awesome's
  `tile.left`); with one tiled window it takes the whole area. New windows join the end
  of the order, so they go to the stack.
- **[Y3] Shares.** Each window in the stack column has a share of its height, equal by
  default; a new window gets an average share. `window_height 0.05` gives the focused
  stack window 5 points more of the column, taken from the others in proportion; no
  share goes below 0.05 or above 0.95.
- **[Y4] Master width.** `master_width 0.05` widens the master column by 5 points of
  the work area; it stays between 0.05 and 0.95.
- **[Y5] Max.** Every tiled window fills the work area; raising one shows it.
- **[Y6] Float.** Windows keep their geometry; switching from another layout leaves
  them where it put them.
- **[Y7] Choosing.** `layouts :tile, :max, :float` sets the cycle and the starting
  layout (its first); `layout :next` and `layout :previous` move along it, wrapping, and
  `layout :max` picks one. `master side: :left, width: 0.6` sets the master column;
  a width outside 0.05 to 0.95 fails the load ([L4]).
- **[Y8] Borders.** A window's border is inside its cell: the window is the cell less
  twice the border width, and at least 1 px.
- **[Y9] Requests.** A tiled window's `ConfigureRequest` is refused with a synthetic
  `ConfigureNotify` of its cell ([M3]); a floating window's is applied, or answered with
  a synthetic one when it asks for where it already is.

## States

- **[W1] Floating.** `toggle :floating` takes a window out of the layout, keeping its
  geometry, or puts it back. A window that starts floating is raised ([W8]).
- **[W2] Placement.** A window that starts floating (in the float layout) without a
  position from its user or program (`WM_NORMAL_HINTS`) is centered on the screen;
  any floating window is moved to stay on screen.
- **[W3] Maximized.** `toggle :maximized` makes a window fill the work area, raised
  ([W8]); toggling again restores its previous state and geometry.
- **[W4] Fullscreen.** `toggle :fullscreen` makes a window cover the whole screen,
  without a border, above every other managed window; toggling again restores it.
- **[W5] On top.** `toggle :on_top` keeps a window above every other, but a fullscreen
  one.
- **[W6] Minimized.** `minimize` unmaps the focused window and sets its `WM_STATE` to
  `IconicState`; the focus goes on as when a window closes ([F5]). `restore` maps the
  most recently minimized window again and focuses it, as a client mapping its own
  minimized window does. An unmap sugoi makes is not the client withdrawing its window
  ([M4]).
- **[W7] Center.** `center` centers the focused window on the screen, and
  `center size: 0.7` also sizes it to 70 % of the screen; both make a tiled window
  float.
- **[W8] Stacking.** From the bottom: every other window, tiled, floating or maximized,
  in the order it was last raised; then the windows on top; then the fullscreen
  window. Raising a window ([F1], [F3], [D5], [W1], [W3]) puts it above the others of
  its layer, so a clicked tiled window covers a floating one, as in awesome; on top
  keeps a window above them ([W5]).

## Mouse

- **[G1] Move.** `move` in a button block starts dragging the clicked window, even one
  that cannot take the focus, until the button is released. A floating window follows
  the pointer, and a maximized or fullscreen one starts floating where it shows. A
  tiled window stays in the layout: in tile, it trades places in the window order
  ([D5]) with each tiled window the pointer enters, as `swap` does, as awesome's tiled
  move does. The pointer shows the move cursor (`fleur`) meanwhile.
- **[G2] Resize.** `resize` in a button block resizes the clicked floating window from
  its corner nearest the pointer, where the pointer is moved, until the button is
  released; the opposite corner stays, and the window keeps at least 1 px. A maximized
  or fullscreen window starts floating. The pointer shows that corner's cursor
  (`top_left_corner` and so on) meanwhile.
- **[G3] Resizing the layout.** In tile, `resize` on a tiled window moves the pointer to
  the edge between the columns, at the window's bottom edge (its top edge if it is the
  last of its column, its middle if it is alone there), and the drag resizes the
  layout, as awesome's tile does: the pointer's x sets the master width, within
  [Y4]'s bounds, and its y moves that edge of the window, the other windows of its
  column sharing the rest in proportion, within [Y3]'s bounds. The window stays tiled;
  the pointer shows `cross`. In max, `resize` and `move` do nothing to a tiled window.

## Focus

- **[Z1] Arranging moves no focus.** A window that moves under the resting pointer
  because sugoi arranged or restacked does not take the focus ([F2]).
- **[Z2] Closing.** When the focused window is unmanaged or minimized and the pointer
  then rests over a managed window that can take the focus, that window gets it; else
  the focus history decides ([F5]).
