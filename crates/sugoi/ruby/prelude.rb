# The DSL of sugoi's config (doc/spec/config.md), loaded once into the embedded Ruby.
# sugoi calls Sugoi.load, Sugoi.commit and Sugoi.run; every other Ruby object stays here.

require "timeout"

module Sugoi
  # [R2] Seconds a block may run.
  TIME_LIMIT = 1.0

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

    # What sugoi reads: [width, focused, normal, modkey, keys, buttons].
    def export
      [@border[:width], @border[:focused], @border[:normal], @modkey, @keys.to_a, @buttons.to_a]
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
      config.instance_eval(File.read(path), path, 1)
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

    def direction(name)
      name = name.to_s
      return name if %w[next previous].include?(name)

      raise ArgumentError, "direction must be :next or :previous, not #{name.inspect}"
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
