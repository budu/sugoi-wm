# Popups

The source of truth for `sugoi-popup`, which shows a bar widget's details as awesome's
widget popups did. It is not part of the WM: polybar runs it on a click, as a small X
client of its own. Tests cite the identifiers in brackets.

- **[O1] Content.** `sugoi-popup` reads Pango markup on its standard input and shows
  it in `monospace 10` at the X resources' DPI (`Xft.dpi`, else 96): text `#aaaaaa` on
  `#222222`, 12 px margins and a 1 px `#aa007b` border, both scaled by the DPI over 96.
  Markup Pango cannot parse shows as plain text; empty input shows nothing, and it
  exits 0. It needs a 24-bit TrueColor screen, and exits 1 on any other.
- **[O2] Place.** The popup is override-redirect, so sugoi does not manage it ([M2]).
  Its top is 5 px (scaled) below the work area's (`_NET_WORKAREA`, else the screen's
  top), and it is centered on the pointer, kept on screen. It is no larger than the
  work area below that top: what does not fit is cut, inside its border.
- **[O3] Close.** It holds the pointer and the keyboard while it shows: a button press
  anywhere or any key closes it, and goes nowhere else, so clicking the widget again
  closes it. It exits 0 then. If it cannot hold them within 1 s (another client does),
  it exits 1 without showing.
- **[O4] Widgets.** `contrib/polybar/status --popup <command...>` runs the widget's
  script, drops its first line (the label) and a `@@POPUP@@` line, and pipes the rest
  to `sugoi-popup`; `--then <command...>` appends another command's output after a
  blank line, as the claude popup appends its sessions. A script that fails shows its
  error in `#f7768e`.
