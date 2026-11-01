# awesome parity

Every binding and rule in the awesome config (`~/.config/awesome/`, read 2026-10-31), and
its sugoi equivalent. The 0.1 plan's Parity measure checks this list at the release.
Status: the milestone that delivers it, or why it is dropped. `doc/config.rb` ports
everything that works today.

## Global keys (30)

| awesome                      | sugoi                                  | Status          |
|------------------------------|----------------------------------------|-----------------|
| Mod-j, Mod-k focus by index  | `focus :previous`, `focus :next`       | M2              |
| Mod-Shift-j, k swap by index | `swap :previous`, `swap :next`         | M2              |
| Mod-Tab toggle-xkbmap        | `sh`; chords follow the new keymap     | M2              |
| Mod-Return kitty             | `spawn "kitty"`                        | M2              |
| Mod-Ctrl-Return superkitty   | `spawn`, the chord's only meaning      | M2, decided     |
| Mod-Ctrl-r restart           | `reload`, or `restart` for a new build | M2              |
| Mod-Shift-q quit (with no focus) | Mod-Shift-q closes the window      | Dropped, decided |
| Mod-Shift-e quit, dmenu confirm | dmenu, then `sugoictl quit`         | M7              |
| Mod-y overmind restart       | `sh`                                   | M2              |
| Mod-d, e, i, b apps          | `spawn` thunar, emacs, chromium, ...   | M2              |
| Mod-z zeditor                | none                                   | Dropped, no Zed |
| Mod-p menubar                | `spawn "rofi", "-show", "drun"`        | M2              |
| Mod-; and Mod-h master width | master width factor                    | M3              |
| Mod-Shift-n, i client height | per-client height factor               | M3              |
| Mod-space, Mod-Shift-space   | next and previous layout               | M3              |
| Mod-Ctrl-n restore minimized | restore                                | M3              |
| Mod-Left, Mod-Right tags     | previous and next workspace            | M4              |
| Mod-Escape tag history       | previous workspace                     | M4              |
| Mod-u urgent client          | none                                   | Dropped, unused |
| Mod-Ctrl--, = column count   | none                                   | Dropped, unused |

The last three rows are under "unused yet" in `keybindings.lua`. The dmenu confirmations
(quit and kill) block on the user, longer than a block may run ([R2]); a script can ask
and then call `sugoictl` once IPC exists (M7).

## Client keys (9)

| awesome                      | sugoi                                  | Status          |
|------------------------------|----------------------------------------|-----------------|
| Mod-Shift-q kill, confirm    | dmenu, then `sugoictl close`           | M7              |
| Mod-c center                 | center                                 | M3              |
| Mod-f fullscreen             | fullscreen                             | M3              |
| Mod-Ctrl-space float toggle  | float toggle                           | M3              |
| Mod-t on top                 | on top                                 | M3              |
| Mod-n minimize               | minimize                               | M3              |
| Mod-m maximize               | maximize                               | M3              |
| Mod-Ctrl-c 70 % centered     | 70 % centered resize                   | M3              |
| Mod-Ctrl-Return swap master  | none                                   | Dropped, decided |

## Tag keys (4 × 9)

| awesome                      | sugoi                                  | Status          |
|------------------------------|----------------------------------------|-----------------|
| Mod-#n view tag              | Mod-n with cycling over 3 banks        | M4              |
| Mod-Shift-#n move to tag     | Mod-Shift-n with the same cycling      | M4              |
| Mod-Ctrl-#n toggle view      | none: one workspace visible            | Dropped, decided |
| Mod-Ctrl-Shift-#n toggle tag | none: one workspace per window         | Dropped, decided |

## Mouse (6)

| awesome                      | sugoi                                  | Status          |
|------------------------------|----------------------------------------|-----------------|
| Client 1 focus and raise     | [F3]                                   | M1              |
| Client Mod-1 move            | Mod-drag moves                         | M3              |
| Client Mod-3 resize          | Mod-right-drag resizes                 | M3              |
| Root 3 main menu             | `button("3") { spawn "rofi", ... }`    | M2              |
| Root 4, 5 next, previous tag | `button` with the workspace actions    | M4              |

## Rules (9)

| awesome                      | sugoi                                  | Status          |
|------------------------------|----------------------------------------|-----------------|
| Default: border, focus, keys | [M1], [F1], [D3], [D4]                 | M1, M2          |
| Default: new clients as slave | new windows join the stack            | M3              |
| Soffice float, 2000×1200, center | rule by class                      | M5              |
| Caprine, Thunar float        | rule by class                          | M5              |
| feh float, 2000×1200, center | rule by class                          | M5              |
| Browsers: 2nd on a tag floats | `on :manage` hook with a query        | M5              |
| Emacs, Cursor as master      | rule by class                          | M5              |
| Zed as master (FIXME)        | none                                   | Dropped, no Zed |

## Outside bindings and rules

- **Sloppy focus** (`signals.lua`): [F2], M1.
- **Error handling** (`error-handling.lua`): [L4] and [R1], M2.
- **Wibar and widgets**: polybar, M7: `contrib/polybar/` keeps the taglist (current
  and occupied workspaces), the title, the tray, clock, CPU, volume and keyboard
  layout, and the claude, codex and update widgets through their scripts; their
  popups become dunst notifications. The tasklist is dropped, as decided.
- **Autostart** of `lxpolkit`: `contrib/sugoi-session`, with dunst and polybar.
