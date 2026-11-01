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
    /// After an accept error (too many open files), the listener rests a while, or it
    /// would stay readable and spin the loop.
    paused_until: Option<Instant>,
    next_id: u64,
}

/// A client reads its request, then gets its reply, both without blocking sugoi.
pub(super) struct Client {
    id: u64,
    pub(super) stream: UnixStream,
    request: Vec<u8>,
    /// The reply and how much of it is written; `None` while the request comes in.
    reply: Option<(Vec<u8>, usize)>,
    deadline: Instant,
}

impl Client {
    /// What the client waits for: to send, or to be sent to.
    pub(super) fn interest(&self) -> rustix::event::PollFlags {
        if self.reply.is_some() {
            rustix::event::PollFlags::OUT
        } else {
            rustix::event::PollFlags::IN
        }
    }
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
                Some(Server { listener, path, clients: Vec::new(), paused_until: None, next_id: 0 })
            }
            Err(e) => {
                warn!("no IPC socket at {}: {e}", path.display());
                None
            }
        }
    }

    /// The nearest deadline, for the event loop's wait.
    pub(super) fn timeout(&self) -> Option<Duration> {
        let now = Instant::now();
        let deadlines = self.clients.iter().map(|c| c.deadline).chain(self.paused_until);
        deadlines.map(|d| d.saturating_duration_since(now)).min()
    }

    /// Whether the listener belongs in the event loop's wait.
    pub(super) fn accepting(&self) -> bool {
        self.paused_until.is_none_or(|until| Instant::now() >= until)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Wm {
    /// [I2] [I4] Accepts clients, reads requests, answers the complete ones and writes
    /// replies as far as the sockets take them; clients past their deadline are dropped.
    pub(super) fn serve_ipc(&mut self) -> Result<(), Error> {
        let Some(server) = self.ipc.as_mut() else { return Ok(()) };
        let now = Instant::now();
        if server.accepting() {
            server.paused_until = None;
            loop {
                match server.listener.accept() {
                    Ok((stream, _)) => {
                        if stream.set_nonblocking(true).is_ok() {
                            server.next_id += 1;
                            server.clients.push(Client {
                                id: server.next_id,
                                stream,
                                request: Vec::new(),
                                reply: None,
                                deadline: now + DEADLINE,
                            });
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    // Out of descriptors: the listener stays readable, so rest it.
                    Err(e) if matches!(e.raw_os_error(), Some(23 | 24)) => {
                        warn!("IPC accept: {e}");
                        server.paused_until = Some(now + DEADLINE);
                        break;
                    }
                    // A connection that failed alone (aborted, reset): take the next.
                    Err(e) => warn!("IPC accept: {e}"),
                }
            }
        }
        // Requests the clients finished sending: they shut their writing side.
        let mut complete = Vec::new();
        for client in server.clients.iter_mut().filter(|c| c.reply.is_none()) {
            let mut bytes = [0; 4096];
            loop {
                match client.stream.read(&mut bytes) {
                    Ok(0) => {
                        complete.push((client.id, std::mem::take(&mut client.request)));
                        break;
                    }
                    Ok(n) => client.request.extend_from_slice(&bytes[..n]),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => {
                        client.deadline = now;
                        break;
                    }
                }
            }
        }
        for (id, request) in complete {
            let reply = encode(&self.answer(&String::from_utf8_lossy(&request))?);
            let server = self.ipc.as_mut().expect("the server is still there");
            if let Some(client) = server.clients.iter_mut().find(|c| c.id == id) {
                client.reply = Some((reply, 0));
                client.deadline = Instant::now() + DEADLINE;
            }
        }
        let server = self.ipc.as_mut().expect("the server is still there");
        let now = Instant::now();
        server.clients.retain_mut(|client| {
            if let Some((reply, written)) = &mut client.reply {
                while *written < reply.len() {
                    match client.stream.write(&reply[*written..]) {
                        // [I4] The limit is on idle time: progress extends it.
                        Ok(n) => {
                            *written += n;
                            client.deadline = now + DEADLINE;
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(_) => return false,
                    }
                }
                if *written == reply.len() {
                    return false;
                }
            }
            now < client.deadline
        });
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
