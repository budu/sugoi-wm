//! sugoi, a non-reparenting X11 window manager. See `doc/spec/`.

mod chord;
mod keyboard;
pub mod layout;
pub mod ruby;
pub mod spawn;
mod wm;
pub mod workspace;

use std::fmt;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;

pub use layout::{Layout, Master, Rect, Side};
pub use wm::Wm;

/// [F6] and [D1]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Border {
    pub width: u16,
    pub focused: u32,
    pub normal: u32,
}

impl Default for Border {
    /// The awesome config's values.
    fn default() -> Self {
        Border { width: 3, focused: 0xaa007b, normal: 0x000000 }
    }
}

/// What a binding does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Built-in commands, for the defaults ([L3]).
    Commands(Vec<Command>),
    /// The config's block with this index ([D3], [D4]).
    Script(usize),
}

/// A chord as written in the config, and its action.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub chord: String,
    pub action: Action,
}

impl Binding {
    pub fn commands(chord: &str, commands: Vec<Command>) -> Binding {
        Binding { chord: chord.to_owned(), action: Action::Commands(commands) }
    }
}

/// A loaded configuration, before sugoi checks its chords.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub border: Border,
    /// [D2] What `Mod` stands for.
    pub modkey: String,
    pub keys: Vec<Binding>,
    /// [D4] Chords on managed windows.
    pub buttons: Vec<Binding>,
    /// [D6] Chords on the root window.
    pub root_buttons: Vec<Binding>,
    /// [Y7] The layout cycle; the first is the starting layout. Never empty.
    pub layouts: Vec<Layout>,
    pub master: Master,
    /// [U2] to [U4] In config order, after the built-in ones ([U5]).
    pub rules: Vec<Rule>,
    /// [H1] Blocks by event, in config order.
    pub hooks: Vec<(Event, usize)>,
}

impl Default for Settings {
    /// [L3]
    fn default() -> Self {
        let xterm = Command::Spawn {
            name: "xterm".into(),
            argv: vec!["xterm".into()],
            limits: Limits::default(),
        };
        Settings {
            border: Border::default(),
            modkey: "Mod4".into(),
            keys: vec![
                Binding::commands("Mod-Return", vec![xterm]),
                Binding::commands("Mod-j", vec![Command::Focus(Direction::Previous)]),
                Binding::commands("Mod-k", vec![Command::Focus(Direction::Next)]),
                Binding::commands("Mod-Shift-q", vec![Command::CloseFocused]),
                Binding::commands("Mod-Shift-e", vec![Command::Quit]),
                Binding::commands("Mod-Ctrl-r", vec![Command::Reload]),
            ]
            .into_iter()
            // [V4] [V8] The keys 1 to 0 are keycodes 10 to 19 on most layouts.
            .chain((1..=10u32).flat_map(|n| {
                [
                    Binding::commands(
                        &format!("Mod-#{}", 9 + n),
                        vec![Command::Workspace(Goto::Number(n))],
                    ),
                    Binding::commands(&format!("Mod-Shift-#{}", 9 + n), vec![Command::MoveTo(n)]),
                ]
            }))
            .collect(),
            buttons: Vec::new(),
            root_buttons: Vec::new(),
            layouts: vec![Layout::Tile, Layout::Max, Layout::Float],
            master: Master::default(),
            rules: Vec::new(),
            hooks: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Next,
    Previous,
}

/// [Y7] A layout to switch to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Next,
    Previous,
    Pick(Layout),
}

/// [V4] to [V6] A workspace to switch to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goto {
    /// 1 to 10 cycle through their class; 11 to 30 are exact.
    Number(u32),
    /// [E9] A workspace index, 0 to 29, whatever the current one.
    Index(usize),
    Next,
    Previous,
    Back,
}

/// A state `toggle` turns on or off ([W1] to [W5]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Floating,
    Maximized,
    Fullscreen,
    OnTop,
}

/// An action sent to a running WM, by a binding or through its [`Handle`].
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// [D5]
    Focus(Direction),
    /// [D5]
    Swap(Direction),
    /// [C1]
    CloseFocused,
    /// [P1]
    Spawn { name: String, argv: Vec<String>, limits: Limits },
    /// [A4]
    Kill,
    /// [L5]
    Reload,
    /// [D5]
    Restart,
    /// [C2]
    Quit,
    /// [Y7]
    Layout(Choice),
    /// [Y4] A change of the master width.
    MasterWidth(f32),
    /// [Y3] A change of the focused window's share.
    WindowHeight(f32),
    /// [W1] to [W5]
    Toggle(State),
    /// [W6]
    Minimize,
    /// [W6]
    Restore,
    /// [W7] With a size, as a fraction of the screen.
    Center(Option<f32>),
    /// [G1]
    Move,
    /// [G2]
    Resize,
    /// [V4] to [V6]
    Workspace(Goto),
    /// [V8]
    MoveTo(u32),
    /// [H2] An action on one window, from a hook.
    On(u32, WindowCommand),
}

/// [H2] What a hook's window can do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowCommand {
    /// [W1] Float, or go back to the layout.
    Float(bool),
    Size(u32, u32),
    Center,
    Master,
    MoveTo(u32),
    Close,
    /// [A4]
    Kill,
}

/// [A1] Resource limits for a spawned app's scope; checked by the config.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    /// `MemoryMax=`: bytes, or a number with K, M, G or T.
    pub memory: Option<String>,
    /// `CPUWeight=`: 1 to 10000.
    pub cpu_weight: Option<u32>,
}

/// [U1]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowType {
    Normal,
    Dialog,
    Splash,
    Utility,
    Toolbar,
    Menu,
    /// [E4] A bar: mapped, never managed.
    Dock,
}

impl WindowType {
    pub const ALL: [(WindowType, &'static str); 7] = [
        (WindowType::Normal, "normal"),
        (WindowType::Dialog, "dialog"),
        (WindowType::Splash, "splash"),
        (WindowType::Utility, "utility"),
        (WindowType::Toolbar, "toolbar"),
        (WindowType::Menu, "menu"),
        (WindowType::Dock, "dock"),
    ];

    pub fn name(self) -> &'static str {
        WindowType::ALL.iter().find(|(t, _)| *t == self).map_or("normal", |(_, n)| n)
    }

    pub fn from_name(name: &str) -> Option<WindowType> {
        WindowType::ALL.iter().find(|(_, n)| *n == name).map(|(t, _)| *t)
    }
}

/// [U1] What a window is, as rules and hooks see it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub class: String,
    pub instance: String,
    pub title: String,
    pub kind: Option<WindowType>,
    pub transient: bool,
    /// [A2] The process of the window's X connection, for a local client.
    pub pid: Option<u32>,
}

impl Info {
    /// [U1] `:normal` without a known type.
    pub fn kind(&self) -> WindowType {
        self.kind.unwrap_or(WindowType::Normal)
    }
}

/// [U2] Empty lists match anything.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Matcher {
    pub class: Vec<String>,
    pub instance: Vec<String>,
    pub title: Vec<String>,
    pub types: Vec<WindowType>,
    pub transient: Option<bool>,
}

impl Matcher {
    pub fn matches(&self, info: &Info) -> bool {
        let any = |list: &[String], value: &str| list.is_empty() || list.iter().any(|v| v == value);
        any(&self.class, &info.class)
            && any(&self.instance, &info.instance)
            && any(&self.title, &info.title)
            && (self.types.is_empty() || self.types.contains(&info.kind()))
            && self.transient.is_none_or(|t| t == info.transient)
    }
}

/// [U3] What a rule sets; `None` and `false` leave a property to other rules.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    pub float: Option<bool>,
    pub size: Option<(u32, u32)>,
    pub center: bool,
    pub on_top: Option<bool>,
    pub master: bool,
    pub workspace: Option<u32>,
}

impl Effects {
    /// [U4] `later` replaces what it sets.
    pub fn then(self, later: &Effects) -> Effects {
        Effects {
            float: later.float.or(self.float),
            size: later.size.or(self.size),
            center: self.center || later.center,
            on_top: later.on_top.or(self.on_top),
            master: self.master || later.master,
            workspace: later.workspace.or(self.workspace),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub matcher: Matcher,
    pub effects: Effects,
}

impl Rule {
    /// [U5] Transients, dialogs, splash screens and utility windows float.
    pub fn defaults() -> Vec<Rule> {
        let float = Effects { float: Some(true), ..Effects::default() };
        let types = vec![WindowType::Dialog, WindowType::Splash, WindowType::Utility];
        vec![
            Rule {
                matcher: Matcher { transient: Some(true), ..Matcher::default() },
                effects: float.clone(),
            },
            Rule { matcher: Matcher { types, ..Matcher::default() }, effects: float },
        ]
    }
}

/// [H1]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Manage,
    Focus,
    Unmanage,
    Workspace,
}

/// [H2] A window as a hook sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub id: u32,
    pub info: Info,
    /// [A3] [A5]
    pub unit: Option<String>,
    /// 1 to 30.
    pub workspace: u32,
    pub floating: bool,
}

/// [H1] What a hook gets.
#[derive(Clone, Debug, PartialEq)]
pub enum HookArg {
    Window(Snapshot),
    Workspace(u32),
}

/// The configuration's code: loads settings and runs the blocks bindings point to.
pub trait Script {
    /// [L4] An error message keeps the previous settings.
    fn load(&mut self) -> Result<Settings, String>;
    /// Makes the settings from the last successful `load` the ones `run` uses.
    fn commit(&mut self) {}
    /// [R1] and [R2] Runs a block; an error message means none of its commands run.
    fn run(&mut self, block: usize) -> Result<Vec<Command>, String>;
    /// [H1] to [H4] Runs a hook's block with its argument and the managed windows.
    fn hook(
        &mut self,
        block: usize,
        _arg: &HookArg,
        _windows: &[Snapshot],
    ) -> Result<Vec<Command>, String> {
        Err(format!("no hook block {block}"))
    }
    /// [I2] Runs a request's Ruby: its value as `inspect` gives it, and its commands.
    fn eval(
        &mut self,
        _code: &str,
        _windows: &[Snapshot],
    ) -> Result<(String, Vec<Command>), String> {
        Err("this sugoi runs without Ruby".into())
    }
}

/// [L3] The built-in settings, without Ruby.
pub struct Defaults;

impl Script for Defaults {
    fn load(&mut self) -> Result<Settings, String> {
        Ok(Settings::default())
    }

    fn run(&mut self, block: usize) -> Result<Vec<Command>, String> {
        Err(format!("the defaults have no block {block}"))
    }
}

/// Sends commands to a running WM from any thread.
#[derive(Clone)]
pub struct Handle {
    commands: Sender<Command>,
    wake: Arc<UnixStream>,
}

impl Handle {
    /// Queues a command; returns false if the WM has stopped.
    pub fn send(&self, command: Command) -> bool {
        self.commands.send(command).is_ok() && (&*self.wake).write_all(&[1]).is_ok()
    }
}

#[derive(Debug)]
pub enum Error {
    /// [S1]
    AnotherWm(String),
    /// [S3]
    Connection(String),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::AnotherWm(display) => {
                write!(f, "another window manager is running on {display}")
            }
            Error::Connection(cause) => write!(f, "X connection lost: {cause}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<x11rb::errors::ConnectionError> for Error {
    fn from(e: x11rb::errors::ConnectionError) -> Self {
        Error::Connection(e.to_string())
    }
}

impl From<x11rb::errors::ReplyOrIdError> for Error {
    fn from(e: x11rb::errors::ReplyOrIdError) -> Self {
        Error::Connection(e.to_string())
    }
}

impl From<x11rb::errors::ConnectError> for Error {
    fn from(e: x11rb::errors::ConnectError) -> Self {
        Error::Connection(e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
