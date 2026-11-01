//! sugoi, a non-reparenting X11 window manager. See `doc/spec/`.

mod chord;
mod keyboard;
pub mod ruby;
pub mod spawn;
mod wm;

use std::fmt;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Built-in commands, for the defaults ([L3]).
    Commands(Vec<Command>),
    /// The config's block with this index ([D3], [D4]).
    Script(usize),
}

/// A chord as written in the config, and its action.
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub border: Border,
    /// [D2] What `Mod` stands for.
    pub modkey: String,
    pub keys: Vec<Binding>,
    pub buttons: Vec<Binding>,
}

impl Default for Settings {
    /// [L3]
    fn default() -> Self {
        let xterm = Command::Spawn { name: "xterm".into(), argv: vec!["xterm".into()] };
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
            ],
            buttons: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Next,
    Previous,
}

/// An action sent to a running WM, by a binding or through its [`Handle`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// [D5]
    Focus(Direction),
    /// [D5]
    Swap(Direction),
    /// [C1]
    CloseFocused,
    /// [P1]
    Spawn { name: String, argv: Vec<String> },
    /// [L5]
    Reload,
    /// [D5]
    Restart,
    /// [C2]
    Quit,
}

/// The configuration's code: loads settings and runs the blocks bindings point to.
pub trait Script {
    /// [L4] An error message keeps the previous settings.
    fn load(&mut self) -> Result<Settings, String>;
    /// Makes the settings from the last successful `load` the ones `run` uses.
    fn commit(&mut self) {}
    /// [R1] and [R2] Runs a block; an error message means none of its commands run.
    fn run(&mut self, block: usize) -> Result<Vec<Command>, String>;
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
