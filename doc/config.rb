# sugoi config ported from ~/.config/awesome (doc/plans/0.1/parity.md).
# Copy to ~/.config/sugoi/config.rb. Later milestones fill in what is missing.

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
key("Mod-Shift-e") { quit }
key("Mod-Shift-q") { close }
key("Mod-Tab") { spawn "sh", "/home/budu/bin/toggle-xkbmap", name: "toggle_xkbmap" }

# Programs, each in its own scope (app-<name>.slice)
key("Mod-Return") { spawn "/usr/bin/kitty", name: "kitty" }
key("Mod-Ctrl-Return") do
  spawn "/usr/bin/bash", "-ic", "~/bin/superkitty", name: "superkitty"
end
key("Mod-d") { spawn "/usr/bin/thunar", name: "thunar" }
key("Mod-e") { spawn "/usr/bin/emacs", name: "emacs" }
key("Mod-z") { spawn "/usr/bin/zeditor", name: "zeditor" }
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

# Mouse
button("Mod-1") { move }
button("Mod-3") { resize }

# The root window's menu
root_button("3") { spawn "rofi", "-show", "drun" }
