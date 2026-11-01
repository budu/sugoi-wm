//! The IPC socket ([I1] to [I4] of `doc/spec/ewmh.md`), served from the event loop.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use log::{info, warn};
use sugoictl::{Reply, encode};

use super::Wm;
use crate::{Error, Snapshot};

/// [I4] How long a client may take to send its request.
const DEADLINE: Duration = Duration::from_secs(1);

pub(super) struct Server {
    pub(super) listener: UnixListener,
    pub(super) path: PathBuf,
    pub(super) clients: Vec<Client>,
}

pub(super) struct Client {
    pub(super) stream: UnixStream,
    request: Vec<u8>,
    deadline: Instant,
}

impl Server {
    /// [I1] Listens on the display's socket, readable and writable by its user only;
    /// `None`, logged, when there is no runtime directory or the socket cannot be made.
    pub(super) fn open(display: &str) -> Option<Server> {
        let path = sugoictl::socket_path(display)?;
        let open = || -> std::io::Result<UnixListener> {
            let dir = path.parent().expect("the socket is in a directory");
            std::fs::create_dir_all(dir)?;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            // sugoi is the display's only WM ([S1]): a socket left there is stale.
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;
            Ok(listener)
        };
        match open() {
            Ok(listener) => {
                info!("listening on {}", path.display());
                Some(Server { listener, path, clients: Vec::new() })
            }
            Err(e) => {
                warn!("no IPC socket at {}: {e}", path.display());
                None
            }
        }
    }

    /// The nearest client deadline, for the event loop's wait.
    pub(super) fn timeout(&self) -> Option<Duration> {
        let now = Instant::now();
        self.clients.iter().map(|c| c.deadline.saturating_duration_since(now)).min()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Wm {
    /// [I2] [I4] Accepts clients, reads what they sent, and answers those done sending;
    /// clients past their deadline are dropped.
    pub(super) fn serve_ipc(&mut self) -> Result<(), Error> {
        let Some(server) = self.ipc.as_mut() else { return Ok(()) };
        while let Ok((stream, _)) = server.listener.accept() {
            if stream.set_nonblocking(true).is_ok() {
                let deadline = Instant::now() + DEADLINE;
                server.clients.push(Client { stream, request: Vec::new(), deadline });
            }
        }
        let mut done = Vec::new();
        let now = Instant::now();
        server.clients.retain_mut(|client| {
            let mut bytes = [0; 4096];
            loop {
                match client.stream.read(&mut bytes) {
                    // The client shut its writing side: the request is complete.
                    Ok(0) => {
                        done.push((client.stream.try_clone(), std::mem::take(&mut client.request)));
                        return false;
                    }
                    Ok(n) => client.request.extend_from_slice(&bytes[..n]),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => return now < client.deadline,
                    Err(_) => return false,
                }
            }
        });
        for (stream, request) in done {
            let reply = self.answer(&String::from_utf8_lossy(&request))?;
            if let Ok(mut stream) = stream {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_write_timeout(Some(DEADLINE));
                let _ = stream.write_all(&encode(&reply));
            }
        }
        Ok(())
    }

    /// [I2] Runs a request's Ruby, then its actions, as for a key.
    fn answer(&mut self, code: &str) -> Result<Reply, Error> {
        let windows: Vec<Snapshot> = self.order.iter().map(|&w| self.snapshot(w)).collect();
        match self.script.eval(code, &windows) {
            Ok((value, commands)) => {
                for command in commands {
                    self.execute(command)?;
                }
                Ok(Reply::Ok(value))
            }
            Err(message) => Ok(Reply::Error(message)),
        }
    }

    /// [I1] Where the socket is, for the programs sugoi starts.
    pub(super) fn socket(&self) -> Option<&std::path::Path> {
        self.ipc.as_ref().map(|server| server.path.as_path())
    }
}
