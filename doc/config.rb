# sugoi config ported from ~/.config/awesome (doc/plans/0.1/parity.md).
# Copy to ~/.config/sugoi/config.rb. Later milestones fill in what is missing.

require "shellwords"

border width: 3, focused: "#aa007b", normal: "#000000"
modkey "Mod4"

# awesome's tile.left: the master on the right
layouts :tile, :max, :float
master side: :right, width: 0.5

# Focus and order
key("Mod-j") { focus :previous }
key("Mod-k") { focus :next }
key("Mod-Shift-j") { swap :previous }
key("Mod-Shift-k") { swap :next }

# Session
key("Mod-Ctrl-r") { reload }
# Asked with dmenu, as awesome did; sugoictl (on PATH) carries the answer out
def confirm(question, request)
  "printf 'No\\nYes' | dmenu -p #{question} | grep -qx Yes && sugoictl #{request}"
end
key("Mod-Shift-e") { sh confirm("Quit?", "quit") }
key("Mod-Shift-q") { sh confirm("Close?", "close") }
key("Mod-Tab") { spawn "sh", "/home/budu/bin/toggle-xkbmap", name: "toggle_xkbmap" }

# Programs, each in its own scope (app-<name>.slice)
key("Mod-Return") { spawn "/usr/bin/kitty", name: "kitty" }
key("Mod-Ctrl-Return") do
  spawn "/usr/bin/bash", "-ic", "~/bin/superkitty", name: "superkitty"
end
key("Mod-d") { spawn "/usr/bin/thunar", name: "thunar" }
key("Mod-e") { spawn "/usr/bin/emacs", name: "emacs" }
key("Mod-i") { spawn "/home/budu/bin/chromium-launcher", name: "chromium" }
key("Mod-b") { spawn "/usr/bin/firefox", name: "firefox" }
key("Mod-p") { spawn "rofi", "-show", "drun" }
key("Mod-y") { sh "bash -ic 'overmind restart -s ~/cg/reservotron/.overmind.sock'" }

# Layouts
key("Mod-space") { layout :next }
key("Mod-Shift-space") { layout :previous }
key("Mod-h") { master_width 0.05 }
key("Mod-semicolon") { master_width(-0.05) }
key("Mod-Shift-i") { window_height 0.05 }
key("Mod-Shift-n") { window_height(-0.05) }

# Window states
key("Mod-c") { center }
key("Mod-Ctrl-c") { center size: 0.7 }
key("Mod-f") { toggle :fullscreen }
key("Mod-m") { toggle :maximized }
key("Mod-t") { toggle :on_top }
key("Mod-Ctrl-space") { toggle :floating }
key("Mod-n") { minimize }
key("Mod-Ctrl-n") { restore }
# Every window, minimized ones too: picking one goes to it and restores it ([E8]).
key("Mod-w") { spawn "rofi", "-show", "window" }
# This workspace's minimized windows; rofi shows the second column, and the pick's ID
# goes back through sugoictl to `activate` ([H2]).
key("Mod-Shift-w") do
  minimized = windows(workspace: current_workspace).select(&:minimized?)
  if minimized.empty?
    sh "notify-send sugoi 'No minimized windows here'"
  else
    lines = minimized.map do |w|
      "#{w.id}\t#{w.class_name}   #{w.title.tr("\t\n", "  ")}"
    end
    rofi = "rofi -dmenu -i -no-custom -p Minimized -display-columns 2 -display-column-separator '\\t'"
    activate = "sugoictl 'windows.find { |w| w.id == {} }&.activate'"
    sh "printf '%s\\n' #{lines.shelljoin} | #{rofi} | cut -f1 | xargs -r -I{} #{activate}"
  end
end

# Workspaces: Mod-1 to Mod-0 by keycode, pressed again to cycle on to 11 to 30
(1..10).each do |n|
  key("Mod-##{9 + n}") { workspace n }
  key("Mod-Shift-##{9 + n}") { move_to n }
end
key("Mod-Left") { workspace :previous }
key("Mod-Right") { workspace :next }
key("Mod-Escape") { workspace :back }
root_button("4") { workspace :next }
root_button("5") { workspace :previous }

# Mouse
button("Mod-1") { move }
button("Mod-3") { resize }

# Rules (rules.lua). Dialogs, transients, splash and utility windows float already.
rule class: "Soffice", float: true, size: [2000, 1200], center: true
rule class: %w[Caprine Thunar], float: true
rule class: "feh", float: true, size: [2000, 1200], center: true
rule class: %w[Emacs Cursor], master: true

# Float browser windows but the first one on a workspace
BROWSERS = %w[Chromium firefox].freeze
on(:manage) do |window|
  next unless BROWSERS.include?(window.class_name)

  here = windows(workspace: window.workspace)
  browsers = here.count { |w| BROWSERS.include?(w.class_name) }
  if browsers > 1
    window.float
    window.size 3000, 1300
    window.center
  end
end

# The root window's menu
root_button("3") { spawn "rofi", "-show", "drun" }
