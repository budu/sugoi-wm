# The DSL of sugoi's config (doc/spec/config.md), loaded once into the embedded Ruby.
# sugoi calls Sugoi.load, Sugoi.commit and Sugoi.run; every other Ruby object stays here.

require "timeout"

# [C3] sugoi handles these signals itself; Ruby's own handlers would only queue them,
# so a config may not trap them either.
%w[TERM INT HUP].each { |signal| trap(signal, "SYSTEM_DEFAULT") }

module Sugoi
  module SignalGuard
    def trap(signal, *args, &block)
      name = signal.is_a?(Integer) ? Signal.signame(signal) : signal.to_s.delete_prefix("SIG")
      raise ArgumentError, "sugoi handles SIG#{name}" if %w[TERM INT HUP].include?(name)

      super
    end
  end
end
[Kernel, Kernel.singleton_class, Signal.singleton_class].each { |m| m.prepend(Sugoi::SignalGuard) }

module Sugoi
  # [R2] Seconds a block may run.
  TIME_LIMIT = 1.0
  # [L6] Seconds a config file may take to load.
  LOAD_LIMIT = 5.0

  # [D5] Actions queue commands; sugoi runs them once the block returns.
  module Actions
    def focus(direction) = Sugoi.queue("focus", Sugoi.direction(direction))
    def swap(direction) = Sugoi.queue("swap", Sugoi.direction(direction))
    def close = Sugoi.queue("close")
    def reload = Sugoi.queue("reload")
    def restart = Sugoi.queue("restart")
    def quit = Sugoi.queue("quit")

    def spawn(*argv, name: nil)
      argv = argv.map { |arg| String(arg) }
      raise ArgumentError, "spawn needs a program" if argv.empty?

      Sugoi.queue("spawn", name.nil? ? File.basename(argv.first) : String(name), *argv)
    end

    def sh(command) = spawn("sh", "-c", String(command), name: "sh")

    # [Y7]
    def layout(name) = Sugoi.queue("layout", Sugoi.one_of(name, %w[next previous tile max float]))
    # [Y4] [Y3]
    def master_width(delta) = Sugoi.queue("master_width", Float(delta).to_s)
    def window_height(delta) = Sugoi.queue("window_height", Float(delta).to_s)
    # [W1] to [W5]
    def toggle(state)
      Sugoi.queue("toggle", Sugoi.one_of(state, %w[floating maximized fullscreen on_top]))
    end
    # [W6]
    def minimize = Sugoi.queue("minimize")
    def restore = Sugoi.queue("restore")
    # [W7]
    def center(size: nil) = Sugoi.queue("center", *(size.nil? ? [] : [Float(size).to_s]))
    # [G1] [G2]
    def move = Sugoi.queue("move")
    def resize = Sugoi.queue("resize")

    # [V4] to [V6]
    def workspace(target)
      return Sugoi.queue("workspace", Sugoi.workspace(target).to_s) if target.is_a?(Integer)

      Sugoi.queue("workspace", Sugoi.one_of(target, %w[next previous back]))
    end

    # [V8]
    def move_to(n) = Sugoi.queue("move_to", Sugoi.workspace(n).to_s)
  end

  # The object a config file runs in.
  class Config
    include Actions

    attr_reader :blocks

    def initialize
      @border = {}
      @modkey = "Mod4"
      @keys = {}
      @buttons = {}
      @root_buttons = {}
      @layouts = %w[tile max float]
      @master = {}
      @blocks = []
    end

    # [D1]
    def border(width: nil, focused: nil, normal: nil)
      @border[:width] = Integer(width) unless width.nil?
      @border[:focused] = Sugoi.color(focused) unless focused.nil?
      @border[:normal] = Sugoi.color(normal) unless normal.nil?
    end

    # [D2]
    def modkey(name) = @modkey = String(name)

    # [D3] A later binding of the same chord replaces the earlier one.
    def key(chord, &block) = @keys[String(chord)] = register(chord, block)

    # [D4]
    def button(chord, &block) = @buttons[String(chord)] = register(chord, block)

    # [D6]
    def root_button(chord, &block) = @root_buttons[String(chord)] = register(chord, block)

    # [Y7]
    def layouts(*names)
      raise ArgumentError, "layouts needs at least one layout" if names.empty?

      @layouts = names.map { |name| Sugoi.one_of(name, %w[tile max float]) }
    end

    # [Y2] [Y4]
    def master(side: nil, width: nil)
      @master[:side] = Sugoi.one_of(side, %w[left right]) unless side.nil?
      @master[:width] = Float(width) unless width.nil?
    end

    # What sugoi reads: [width, focused, normal, modkey, keys, buttons, root buttons,
    # layouts, master side, master width].
    def export
      [@border[:width], @border[:focused], @border[:normal], @modkey,
       @keys.to_a, @buttons.to_a, @root_buttons.to_a,
       @layouts, @master[:side], @master[:width]]
    end

    private

    def register(chord, block)
      raise ArgumentError, "#{chord.inspect} needs a block" unless block

      @blocks << block
      @blocks.size - 1
    end
  end

  class << self
    # [L4] Runs the config at `path`: its exported settings, or an error message.
    def load(path)
      @path = path
      config = Config.new
      Timeout.timeout(LOAD_LIMIT) { config.instance_eval(File.read(path), path, 1) }
      @candidate = config
      config.export
    rescue ScriptError, StandardError => e
      describe(e)
    end

    # Makes the last loaded config the one blocks come from.
    def commit
      @current = @candidate
      nil
    end

    # [R1] [R2] Runs block `index`: its commands, or an error message.
    def run(index)
      @queue = []
      Timeout.timeout(TIME_LIMIT) { @current.blocks.fetch(index).call }
      @queue
    rescue ScriptError, StandardError => e
      describe(e)
    ensure
      @queue = nil
    end

    def queue(*command)
      raise "actions only run inside a key or button block" unless @queue

      @queue << command
      nil
    end

    def direction(name) = one_of(name, %w[next previous])

    def workspace(n)
      return n if n.is_a?(Integer) && (1..30).cover?(n)

      raise ArgumentError, "workspaces are numbered 1 to 30, not #{n.inspect}"
    end

    def one_of(name, names)
      return name.to_s if names.include?(name.to_s)

      raise ArgumentError, "expected one of :#{names.join(", :")}, not #{name.inspect}"
    end

    def color(value)
      hex = String(value).delete_prefix("#")
      raise ArgumentError unless hex.match?(/\A\h{6}\z/)

      hex.to_i(16)
    rescue ArgumentError
      raise ArgumentError, "color must be \"#rrggbb\", not #{value.inspect}"
    end

    private

    # The message on one line, with the config line it comes from.
    def describe(error)
      return describe_syntax(error.message) if error.is_a?(SyntaxError)

      line = error.backtrace_locations&.find { |location| location.path == @path }
      where = line ? "#{line.path}:#{line.lineno}: " : ""
      "#{where}#{error.message} (#{error.class})"
    end

    # Prism lists the source under its first line, with each message after a caret:
    # "path:2: syntax errors found", "> 2 | key ...", "  |   ^ expected ...".
    def describe_syntax(message)
      first, *rest = message.lines(chomp: true)
      notes = rest.filter_map { |line| line[/\^+ (.*)/, 1] }
      location = first.sub(/: syntax errors? found\z/, "")
      notes.empty? ? message.tr("\n", " ") : "#{location}: syntax error: #{notes.join("; ")}"
    end
  end
end
