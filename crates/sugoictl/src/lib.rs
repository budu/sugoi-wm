//! The IPC between sugoi and `sugoictl` ([I1] to [I4] of `doc/spec/ewmh.md`).

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

/// [I1] The variable that names the socket for programs sugoi starts.
pub const SOCKET_VARIABLE: &str = "SUGOI_SOCKET";

/// [I1] `$XDG_RUNTIME_DIR/sugoi/<display>.sock`, the display without its `:` and screen.
pub fn socket_path(display: &str) -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty())?;
    let name = display.trim_start_matches(':');
    let name = name.split('.').next().unwrap_or(name);
    Some(PathBuf::from(runtime).join("sugoi").join(format!("{name}.sock")))
}

/// The socket a client should use: `$SUGOI_SOCKET`, else the one for `$DISPLAY`.
pub fn default_socket() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(SOCKET_VARIABLE).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    socket_path(&std::env::var("DISPLAY").ok()?)
}

/// [I2] What sugoi replied.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    /// The request's value, as `inspect` gives it.
    Ok(String),
    Error(String),
}

/// The reply's wire form: a first line `ok` or `error`, then the text.
pub fn encode(reply: &Reply) -> Vec<u8> {
    match reply {
        Reply::Ok(value) => format!("ok\n{value}").into_bytes(),
        Reply::Error(message) => format!("error\n{message}").into_bytes(),
    }
}

pub fn decode(bytes: &[u8]) -> Option<Reply> {
    let text = String::from_utf8_lossy(bytes);
    let (status, rest) = text.split_once('\n').unwrap_or((&text, ""));
    match status {
        "ok" => Some(Reply::Ok(rest.to_owned())),
        "error" => Some(Reply::Error(rest.to_owned())),
        _ => None,
    }
}

/// [I2] Sends `code` and waits for the reply: the request is everything until the
/// client shuts its writing side.
pub fn request(socket: &std::path::Path, code: &str) -> std::io::Result<Reply> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(code.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    decode(&bytes).ok_or_else(|| std::io::Error::other("sugoi sent no reply"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_survive_the_wire() {
        for reply in [Reply::Ok("[1, 2]".into()), Reply::Error("boom\non two lines".into())] {
            assert_eq!(decode(&encode(&reply)), Some(reply));
        }
        assert_eq!(decode(b"what"), None);
    }

    #[test]
    fn the_socket_is_named_after_the_display() {
        let path = |display| {
            socket_path(display).map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        };
        if std::env::var_os("XDG_RUNTIME_DIR").is_some() {
            assert_eq!(path(":1").as_deref(), Some("1.sock"));
            assert_eq!(path(":12.0").as_deref(), Some("12.sock"));
        }
    }
}
