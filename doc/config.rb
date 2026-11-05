# An example sugoi config: copy it to ~/.config/sugoi/config.rb and make it yours.
# It binds every action sugoi has; doc/spec/config.md and doc/spec/rules.md say what
# each does. Programs it starts are examples: kitty, firefox, rofi, notify-send.

require "shellwords"

# Windows on top (Mod-t) get amber borders, so they are seen ([F6]).
border width: 3, focused: "#aa007b", normal: "#000000",
       on_top: "#7a5c1e", focused_on_top: "#e0af68"
modkey "Mod4"

# Tile with the master on the right, as awesome's tile.left
layouts :tile, :max, :float
master side: :right, width: 0.5

# Focus and order
key("Mod-j") { focus :previous }
key("Mod-k") { focus :next }
key("Mod-Shift-j") { swap :previous }
key("Mod-Shift-k") { swap :next }

# Session
key("Mod-Ctrl-r") { reload }
# Asked with rofi (No first, no text entry); sugoictl (on PATH) carries the answer out
def confirm(question, request)
  theme = "window { width: 16%; } listview { lines: 2; } inputbar { children: [prompt]; }"
  rofi = "rofi -dmenu -i -no-custom -no-show-icons -p #{question.shellescape} " \
         "-theme-str #{theme.shellescape}"
  "printf 'No\\nYes' | #{rofi} | grep -qx Yes && sugoictl #{request}"
end
key("Mod-Shift-e") { sh confirm("Quit?", "quit") }
key("Mod-Shift-q") { sh confirm("Close?", "close") }

# Programs, each in its own systemd scope (app-<name>.slice)
key("Mod-Return") { spawn "kitty" }
key("Mod-b") { spawn "firefox" }
key("Mod-p") { spawn "rofi", "-show", "drun" }
# A program with resource limits for its scope
key("Mod-Ctrl-Return") { spawn "kitty", name: "scratch", memory: "2G", cpu_weight: 50 }

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

# Workspaces: Mod-1 to Mod-0 by keycode, pressed again to cycle on to the stacked ones
# that hold a window (n+10, n+20, [V4]); Mod-Ctrl-n opens a new stacked workspace, and
# Mod-Ctrl-Shift-n moves the focused window to one, following it.
def free_workspace(n)
  [n + 10, n + 20].find { |w| windows(workspace: w).empty? }
end

def no_free_workspace(n) = sh("notify-send sugoi 'No free workspace for #{n}'")

(1..10).each do |n|
  key("Mod-##{9 + n}") { workspace n }
  key("Mod-Shift-##{9 + n}") { move_to n }
  key("Mod-Ctrl-##{9 + n}") do
    free = free_workspace(n)
    free ? workspace(free) : no_free_workspace(n)
  end
  key("Mod-Ctrl-Shift-##{9 + n}") do
    free = free_workspace(n)
    if free
      move_to free
      workspace free
    else
      no_free_workspace(n)
    end
  end
end
key("Mod-Left") { workspace :previous }
key("Mod-Right") { workspace :next }
key("Mod-Escape") { workspace :back }
root_button("4") { workspace :next }
root_button("5") { workspace :previous }

# Mouse: in tile, a drag swaps a tiled window or resizes the layout ([G1] to [G3])
button("Mod-1") { move }
button("Mod-3") { resize }

# The root window's menu
root_button("3") { spawn "rofi", "-show", "drun" }

# Rules. Dialogs, transients, splash and utility windows float already.
rule class: "feh", float: true, size: [1600, 1000], center: true
rule class: "Emacs", master: true

# A hook: a second browser window on a workspace floats, centered
on(:manage) do |window|
  next unless window.class_name == "firefox"

  browsers = windows(workspace: window.workspace).count { |w| w.class_name == "firefox" }
  if browsers > 1
    window.size 1600, 1000
    window.center
  end
end
