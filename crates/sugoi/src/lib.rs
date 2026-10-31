//! sugoi, a non-reparenting X11 window manager. See `doc/spec/core.md`.

mod wm;

use std::fmt;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;

pub use wm::Wm;

/// Appearance and behavior settings; the Ruby config replaces the defaults in M2.
#[derive(Clone, Debug)]
pub struct Config {
    pub border_width: u16,
    pub focused_color: u32,
    pub normal_color: u32,
}

impl Default for Config {
    /// [F6] The awesome config's values.
    fn default() -> Self {
        Config { border_width: 3, focused_color: 0xaa007b, normal_color: 0x000000 }
    }
}

/// An action sent to a running WM through its [`Handle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// [C1]
    CloseFocused,
    /// [C2]
    Quit,
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
