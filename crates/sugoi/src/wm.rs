//! The window manager: X connection, managed windows, focus and the event loop.

use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use log::{debug, info};
use rustix::event::{PollFd, PollFlags, poll};
use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::properties::WmHints;
use x11rb::protocol::xproto::*;
use x11rb::protocol::{ErrorKind, Event};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use crate::{Command, Config, Error, Handle};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        WM_TAKE_FOCUS,
        WM_STATE,
    }
}

/// The `PointerRoot` focus target.
const POINTER_ROOT: Window = 1;
/// `NormalState` in `WM_STATE` (ICCCM 4.1.3.1).
const NORMAL_STATE: u32 = 1;
/// [F3] Buttons that focus on click; the wheel (4 and 5) is not grabbed.
const CLICKS: [ButtonIndex; 3] = [ButtonIndex::M1, ButtonIndex::M2, ButtonIndex::M3];

/// A managed window.
struct Client {
    hints: Hints,
    /// The border width before sugoi managed the window, restored on withdrawal.
    border_width: u16,
}

/// [F4] The ICCCM properties that decide how a window takes focus and closes.
#[derive(Clone, Copy)]
struct Hints {
    /// The `WM_HINTS` input flag, true when absent.
    accepts_input: bool,
    take_focus: bool,
    delete_window: bool,
}

impl Hints {
    fn focusable(self) -> bool {
        self.accepts_input || self.take_focus
    }
}

pub struct Wm {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
    config: Config,
    clients: HashMap<Window, Client>,
    /// [F5] Managed windows, most recently focused first.
    history: Vec<Window>,
    focused: Option<Window>,
    commands: Receiver<Command>,
    wake: UnixStream,
    handle: Handle,
}

impl Wm {
    /// Connects to `display` (or `$DISPLAY`), becomes its WM and adopts its windows.
    pub fn new(display: Option<&str>, config: Config) -> Result<Wm, Error> {
        let (conn, screen) = x11rb::connect(display)?;
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms::new(&conn)?.reply().map_err(lost)?;

        // [S1]
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        let attributes = ChangeWindowAttributesAux::new().event_mask(mask);
        match conn.change_window_attributes(root, &attributes)?.check() {
            Ok(()) => {}
            Err(ReplyError::X11Error(e)) if e.error_kind == ErrorKind::Access => {
                let name = display.map(str::to_owned).or_else(|| std::env::var("DISPLAY").ok());
                return Err(Error::AnotherWm(name.unwrap_or_default()));
            }
            Err(e) => return Err(lost(e)),
        }

        let (sender, commands) = mpsc::channel();
        let (wake, wake_writer) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        let handle = Handle { commands: sender, wake: Arc::new(wake_writer) };
        let mut wm = Wm {
            conn,
            root,
            atoms,
            config,
            clients: HashMap::new(),
            history: Vec::new(),
            focused: None,
            commands,
            wake,
            handle,
        };
        wm.adopt()?;
        wm.conn.flush()?;
        info!("managing {} windows", wm.clients.len());
        Ok(wm)
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    /// Runs until [C2] quits it (`Ok`) or the connection is lost ([S3]).
    pub fn run(mut self) -> Result<(), Error> {
        loop {
            self.conn.flush()?;
            if let Some(event) = self.conn.poll_for_event()? {
                self.handle_event(event)?;
                continue;
            }
            if let Some(command) = self.next_command() {
                if command == Command::Quit {
                    self.conn.flush()?;
                    info!("quit");
                    return Ok(());
                }
                self.run_command(command)?;
                continue;
            }
            let mut fds = [
                PollFd::new(self.conn.stream(), PollFlags::IN),
                PollFd::new(&self.wake, PollFlags::IN),
            ];
            match poll(&mut fds, None) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(e) => return Err(Error::Io(e.into())),
            }
        }
    }

    fn next_command(&mut self) -> Option<Command> {
        let mut bytes = [0; 64];
        while matches!((&self.wake).read(&mut bytes), Ok(n) if n > 0) {}
        self.commands.try_recv().ok()
    }

    fn run_command(&mut self, command: Command) -> Result<(), Error> {
        match command {
            Command::CloseFocused => {
                if let Some(window) = self.focused {
                    self.close(window)?;
                }
            }
            Command::Quit => {}
        }
        Ok(())
    }

    /// [S2]
    fn adopt(&mut self) -> Result<(), Error> {
        let tree = self.conn.query_tree(self.root)?.reply().map_err(lost)?;
        let cookies = tree
            .children
            .iter()
            .map(|&window| Ok((window, self.conn.get_window_attributes(window)?)))
            .collect::<Result<Vec<_>, Error>>()?;
        let mut viewable = Vec::new();
        for (window, cookie) in cookies {
            if let Some(attributes) = vanished(cookie.reply())?
                && attributes.map_state == MapState::VIEWABLE
            {
                viewable.push((window, attributes));
            }
        }
        // Children come bottom to top; the history wants the topmost first.
        for (window, attributes) in viewable.into_iter().rev() {
            self.manage_window(window, &attributes, false)?;
        }
        let pointer = self.conn.query_pointer(self.root)?.reply().map_err(lost)?;
        if self.clients.contains_key(&pointer.child) {
            self.focus(pointer.child, false)?;
        }
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::MapRequest(e) => self.manage(e.window)?,
            Event::ConfigureRequest(e) => self.configure_request(&e)?,
            Event::UnmapNotify(e) if self.clients.contains_key(&e.window) => {
                self.unmanage(e.window, false)?
            }
            Event::DestroyNotify(e) if self.clients.contains_key(&e.window) => {
                self.unmanage(e.window, true)?
            }
            // [F2]
            Event::EnterNotify(e) => {
                if e.mode == NotifyMode::NORMAL
                    && e.detail != NotifyDetail::INFERIOR
                    && self.clients.contains_key(&e.event)
                {
                    self.focus(e.event, false)?;
                }
            }
            // [F3] The grab is synchronous: replay the press so the window gets it.
            Event::ButtonPress(e) => {
                if self.clients.contains_key(&e.event) {
                    self.focus(e.event, true)?;
                }
                self.conn.allow_events(Allow::REPLAY_POINTER, e.time)?;
            }
            Event::PropertyNotify(e) => {
                let hints: Atom = AtomEnum::WM_HINTS.into();
                if (e.atom == hints || e.atom == self.atoms.WM_PROTOCOLS)
                    && self.clients.contains_key(&e.window)
                    && let Some(hints) = self.read_hints(e.window)?
                    && let Some(client) = self.clients.get_mut(&e.window)
                {
                    client.hints = hints;
                }
            }
            // [M5]
            Event::Error(e) => debug!("X error: {e:?}"),
            _ => {}
        }
        Ok(())
    }

    /// [M1] and [M2], for a `MapRequest`.
    fn manage(&mut self, window: Window) -> Result<(), Error> {
        if self.clients.contains_key(&window) {
            self.conn.map_window(window)?;
            return Ok(());
        }
        let Some(attributes) = vanished(self.conn.get_window_attributes(window)?.reply())? else {
            return Ok(());
        };
        self.manage_window(window, &attributes, true)
    }

    /// `map` is false for windows adopted at startup.
    fn manage_window(
        &mut self,
        window: Window,
        attributes: &GetWindowAttributesReply,
        map: bool,
    ) -> Result<(), Error> {
        if attributes.override_redirect {
            if map {
                self.conn.map_window(window)?;
            }
            return Ok(());
        }
        // Select property changes before reading the properties, so none is missed.
        let mask = EventMask::ENTER_WINDOW
            | EventMask::FOCUS_CHANGE
            | EventMask::PROPERTY_CHANGE
            | EventMask::STRUCTURE_NOTIFY;
        let selection = ChangeWindowAttributesAux::new().event_mask(mask);
        self.conn.change_window_attributes(window, &selection)?;
        let geometry = self.conn.get_geometry(window)?;
        let Some(hints) = self.read_hints(window)? else {
            return Ok(());
        };
        let Some(geometry) = vanished(geometry.reply())? else {
            return Ok(());
        };
        debug!("manage {window:#x}");
        let border = ConfigureWindowAux::new().border_width(u32::from(self.config.border_width));
        self.conn.configure_window(window, &border)?;
        let color = ChangeWindowAttributesAux::new().border_pixel(self.config.normal_color);
        self.conn.change_window_attributes(window, &color)?;
        for button in CLICKS {
            self.conn.grab_button(
                false,
                window,
                EventMask::BUTTON_PRESS,
                GrabMode::SYNC,
                GrabMode::ASYNC,
                NONE,
                NONE,
                button,
                ModMask::ANY,
            )?;
        }
        let state = self.atoms.WM_STATE;
        self.conn.change_property32(
            PropMode::REPLACE,
            window,
            state,
            state,
            &[NORMAL_STATE, NONE],
        )?;
        self.clients.insert(window, Client { hints, border_width: geometry.border_width });
        self.history.push(window);
        if map {
            self.conn.map_window(window)?;
            // [F1]
            self.focus(window, true)?;
        }
        Ok(())
    }

    /// [M4]
    fn unmanage(&mut self, window: Window, destroyed: bool) -> Result<(), Error> {
        debug!("unmanage {window:#x}");
        let Some(client) = self.clients.remove(&window) else {
            return Ok(());
        };
        self.history.retain(|&w| w != window);
        if !destroyed {
            // Withdrawn windows may be reused (a tray icon): leave nothing of sugoi's.
            self.conn.delete_property(window, self.atoms.WM_STATE)?;
            self.conn.ungrab_button(ButtonIndex::ANY, window, ModMask::ANY)?;
            let selection = ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT);
            self.conn.change_window_attributes(window, &selection)?;
            let border = ConfigureWindowAux::new().border_width(u32::from(client.border_width));
            self.conn.configure_window(window, &border)?;
        }
        if self.focused == Some(window) {
            self.focused = None;
            // [F5]
            let next = self.history.iter().copied().find(|w| self.clients[w].hints.focusable());
            match next {
                Some(next) => self.focus(next, false)?,
                None => {
                    self.conn.set_input_focus(
                        InputFocus::POINTER_ROOT,
                        POINTER_ROOT,
                        CURRENT_TIME,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// [M3]
    fn configure_request(&mut self, e: &ConfigureRequestEvent) -> Result<(), Error> {
        let mut aux = ConfigureWindowAux::from_configure_request(e);
        let managed = self.clients.contains_key(&e.window);
        if managed {
            aux.border_width = None;
        }
        self.conn.configure_window(e.window, &aux)?;
        let resized =
            aux.x.is_some() || aux.y.is_some() || aux.width.is_some() || aux.height.is_some();
        if managed && !resized {
            let Some(geometry) = vanished(self.conn.get_geometry(e.window)?.reply())? else {
                return Ok(());
            };
            let notify = ConfigureNotifyEvent {
                response_type: CONFIGURE_NOTIFY_EVENT,
                sequence: 0,
                event: e.window,
                window: e.window,
                above_sibling: NONE,
                x: geometry.x,
                y: geometry.y,
                width: geometry.width,
                height: geometry.height,
                border_width: geometry.border_width,
                override_redirect: false,
            };
            self.conn.send_event(false, e.window, EventMask::STRUCTURE_NOTIFY, notify)?;
        }
        Ok(())
    }

    /// [F4] and [F6]; `raise` for new windows and clicks, not for sloppy focus.
    ///
    /// The input focus is set at `CurrentTime`, and protocol messages carry it too: sugoi
    /// has no fresh server timestamp, and an event's stamp can be older than the server's
    /// last focus change, which makes X ignore the request.
    fn focus(&mut self, window: Window, raise: bool) -> Result<(), Error> {
        let Some(client) = self.clients.get(&window) else {
            return Ok(());
        };
        let hints = client.hints;
        if raise {
            let above = ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
            self.conn.configure_window(window, &above)?;
        }
        // Refocusing the focused window would take the focus from its subwindows.
        if self.focused == Some(window) || !hints.focusable() {
            return Ok(());
        }
        if let Some(previous) = self.focused {
            let normal = ChangeWindowAttributesAux::new().border_pixel(self.config.normal_color);
            self.conn.change_window_attributes(previous, &normal)?;
        }
        if hints.accepts_input {
            self.conn.set_input_focus(InputFocus::POINTER_ROOT, window, CURRENT_TIME)?;
        }
        if hints.take_focus {
            self.send_protocol(window, self.atoms.WM_TAKE_FOCUS)?;
        }
        let focused = ChangeWindowAttributesAux::new().border_pixel(self.config.focused_color);
        self.conn.change_window_attributes(window, &focused)?;
        self.history.retain(|&w| w != window);
        self.history.insert(0, window);
        self.focused = Some(window);
        Ok(())
    }

    /// [K1]
    fn close(&mut self, window: Window) -> Result<(), Error> {
        let deletes = self.clients.get(&window).is_some_and(|c| c.hints.delete_window);
        if deletes {
            self.send_protocol(window, self.atoms.WM_DELETE_WINDOW)?;
        } else {
            self.conn.kill_client(window)?;
        }
        Ok(())
    }

    fn send_protocol(&self, window: Window, protocol: Atom) -> Result<(), Error> {
        let data = [protocol, CURRENT_TIME, 0, 0, 0];
        let message = ClientMessageEvent::new(32, window, self.atoms.WM_PROTOCOLS, data);
        self.conn.send_event(false, window, EventMask::NO_EVENT, message)?;
        Ok(())
    }

    /// Reads `WM_HINTS` and `WM_PROTOCOLS`; `None` if the window no longer exists.
    fn read_hints(&self, window: Window) -> Result<Option<Hints>, Error> {
        let wm_hints = WmHints::get(&self.conn, window)?;
        let protocols = self.conn.get_property(
            false,
            window,
            self.atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            0,
            64,
        )?;
        let Some(wm_hints) = vanished(wm_hints.reply())? else {
            return Ok(None);
        };
        let Some(protocols) = vanished(protocols.reply())? else {
            return Ok(None);
        };
        let protocols: Vec<Atom> = protocols.value32().map(|v| v.collect()).unwrap_or_default();
        Ok(Some(Hints {
            accepts_input: wm_hints.and_then(|h| h.input).unwrap_or(true),
            take_focus: protocols.contains(&self.atoms.WM_TAKE_FOCUS),
            delete_window: protocols.contains(&self.atoms.WM_DELETE_WINDOW),
        }))
    }
}

/// [M5] A reply that failed because its window is gone becomes `None`.
fn vanished<T>(reply: Result<T, ReplyError>) -> Result<Option<T>, Error> {
    match reply {
        Ok(value) => Ok(Some(value)),
        Err(ReplyError::X11Error(e)) => {
            debug!("X error: {e:?}");
            Ok(None)
        }
        Err(ReplyError::ConnectionError(e)) => Err(e.into()),
    }
}

fn lost(e: ReplyError) -> Error {
    Error::Connection(e.to_string())
}
