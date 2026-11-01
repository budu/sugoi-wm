//! The window manager: X connection, managed windows, focus and the event loop.

use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use log::{debug, info};
use rustix::event::{PollFd, PollFlags, poll};
use x11rb::connection::Connection;
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::properties::WmHints;
use x11rb::protocol::xproto::*;
use x11rb::protocol::{ErrorKind, Event};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use crate::chord::{self, Key};
use crate::keyboard::Keyboard;
use crate::spawn::{notify, spawn};
use crate::{Action, Border, Command, Defaults, Direction, Error, Handle, Script, Settings};

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

/// Why the event loop ends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Exit {
    Quit,
    Restart,
}

pub struct Wm {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
    script: Box<dyn Script>,
    border: Border,
    keyboard: Keyboard,
    /// [D3] Key chords as configured, bound to keycodes by `grab_keys`.
    key_chords: Vec<(ModMask, Key, Action)>,
    keys: HashMap<(u16, Keycode), Action>,
    /// [D4]
    buttons: HashMap<(u16, u8), Action>,
    clients: HashMap<Window, Client>,
    /// [D5] Managed windows in the order `focus` and `swap` follow.
    order: Vec<Window>,
    /// [F5] Managed windows, most recently focused first.
    history: Vec<Window>,
    focused: Option<Window>,
    exit: Option<Exit>,
    commands: Receiver<Command>,
    wake: UnixStream,
    handle: Handle,
}

impl Wm {
    /// Connects to `display` (or `$DISPLAY`), becomes its WM, loads the settings from
    /// `script` and adopts the windows.
    pub fn new(display: Option<&str>, script: Box<dyn Script>) -> Result<Wm, Error> {
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
        // [D4] Buttons on the root window; selected apart, since another client may
        // hold this selection without being a WM.
        let mask = mask | EventMask::BUTTON_PRESS;
        let attributes = ChangeWindowAttributesAux::new().event_mask(mask);
        conn.change_window_attributes(root, &attributes)?;
        let keyboard = Keyboard::read(&conn)?;

        let (sender, commands) = mpsc::channel();
        let (wake, wake_writer) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        let handle = Handle { commands: sender, wake: Arc::new(wake_writer) };
        let mut wm = Wm {
            conn,
            root,
            atoms,
            script,
            border: Border::default(),
            keyboard,
            key_chords: Vec::new(),
            keys: HashMap::new(),
            buttons: HashMap::new(),
            clients: HashMap::new(),
            order: Vec::new(),
            history: Vec::new(),
            focused: None,
            exit: None,
            commands,
            wake,
            handle,
        };
        wm.load()?;
        wm.adopt()?;
        wm.conn.flush()?;
        info!("managing {} windows", wm.clients.len());
        Ok(wm)
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    /// Runs until [C2] quits it (`Ok`) or the connection is lost ([S3]). A restart
    /// ([D5]) replaces the process and returns only if that fails.
    pub fn run(mut self) -> Result<(), Error> {
        loop {
            if let Some(exit) = self.exit.take() {
                self.release_all()?;
                // A round trip, so the server applies the restores before sugoi exits.
                self.conn.sync().map_err(lost)?;
                match exit {
                    Exit::Quit => {
                        info!("quit");
                        return Ok(());
                    }
                    Exit::Restart => {
                        restart();
                        self.rebind()?;
                    }
                }
            }
            self.conn.flush()?;
            if let Some(event) = self.conn.poll_for_event()? {
                self.handle_event(event)?;
                continue;
            }
            if let Some(command) = self.next_command() {
                self.execute(command)?;
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

    fn execute(&mut self, command: Command) -> Result<(), Error> {
        match command {
            Command::Focus(direction) => {
                if let Some(window) = self.neighbor(direction) {
                    self.focus(window, true)?;
                }
            }
            Command::Swap(direction) => {
                if let (Some(focused), Some(other)) = (self.focused, self.neighbor(direction)) {
                    let a = self.order.iter().position(|&w| w == focused);
                    let b = self.order.iter().position(|&w| w == other);
                    if let (Some(a), Some(b)) = (a, b) {
                        self.order.swap(a, b);
                    }
                }
            }
            Command::CloseFocused => {
                if let Some(window) = self.focused {
                    self.close(window)?;
                }
            }
            Command::Spawn { name, argv } => spawn(&name, &argv),
            Command::Reload => self.load()?,
            Command::Restart => self.exit = Some(Exit::Restart),
            Command::Quit => self.exit = Some(Exit::Quit),
        }
        Ok(())
    }

    /// [D5] The window after or before the focused one in the window order, wrapping.
    fn neighbor(&self, direction: Direction) -> Option<Window> {
        let len = self.order.len();
        let index = match self.focused.and_then(|f| self.order.iter().position(|&w| w == f)) {
            Some(i) if direction == Direction::Next => (i + 1) % len,
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.order.get(index).copied()
    }

    fn run_action(&mut self, action: Action) -> Result<(), Error> {
        let commands = match action {
            Action::Commands(commands) => commands,
            Action::Script(block) => match self.script.run(block) {
                Ok(commands) => commands,
                Err(message) => {
                    report(&message);
                    return Ok(());
                }
            },
        };
        for command in commands {
            self.execute(command)?;
        }
        Ok(())
    }

    /// [L3] [L4] [L5] Loads the settings; on failure the current ones stay.
    fn load(&mut self) -> Result<(), Error> {
        let applied = match self.script.load() {
            Ok(settings) => self.apply(&settings),
            Err(message) => Err(message),
        };
        match applied {
            Ok(()) => self.script.commit(),
            Err(message) => {
                report(&message);
                // [L3] The first load falls back to the defaults.
                if self.key_chords.is_empty() {
                    let defaults = Defaults.load().expect("the defaults load");
                    if let Err(e) = self.apply(&defaults) {
                        report(&format!("the defaults: {e}"));
                    }
                }
            }
        }
        Ok(())
    }

    /// Checks every chord, then makes `settings` current: nothing changes on an error.
    fn apply(&mut self, settings: &Settings) -> Result<(), String> {
        let modkey = chord::modkey(&settings.modkey)?;
        let mut key_chords = Vec::new();
        for binding in &settings.keys {
            let (mods, key) = chord::key(&binding.chord, modkey)?;
            key_chords.push((mods, key, binding.action.clone()));
        }
        let mut buttons = HashMap::new();
        for binding in &settings.buttons {
            let (mods, button) = chord::button(&binding.chord, modkey)?;
            buttons.insert((u16::from(mods), button), binding.action.clone());
        }
        self.key_chords = key_chords;
        self.buttons = buttons;
        self.border = settings.border;
        self.rebind().map_err(|e| e.to_string())
    }

    /// Grabs every key and button chord again, and redraws every border.
    fn rebind(&mut self) -> Result<(), Error> {
        self.grab_keys()?;
        let windows: Vec<Window> = self.clients.keys().copied().collect();
        for window in windows {
            self.conn.ungrab_button(ButtonIndex::ANY, window, ModMask::ANY)?;
            self.grab_buttons(window)?;
            let width = ConfigureWindowAux::new().border_width(u32::from(self.border.width));
            self.conn.configure_window(window, &width)?;
            let color =
                if self.focused == Some(window) { self.border.focused } else { self.border.normal };
            let color = ChangeWindowAttributesAux::new().border_pixel(color);
            self.conn.change_window_attributes(window, &color)?;
        }
        Ok(())
    }

    /// [R4] The lock modifiers a chord matches with: none, Caps Lock, Num Lock, both.
    fn locks(&self) -> Vec<ModMask> {
        let num = self.keyboard.num_lock;
        let mut locks = vec![ModMask::default(), ModMask::LOCK];
        if num != ModMask::default() {
            locks.extend([num, num | ModMask::LOCK]);
        }
        locks
    }

    /// The modifiers of an event's state, without the lock modifiers and buttons.
    fn clean(&self, state: KeyButMask) -> u16 {
        let mods = ModMask::SHIFT
            | ModMask::CONTROL
            | ModMask::M1
            | ModMask::M2
            | ModMask::M3
            | ModMask::M4
            | ModMask::M5;
        u16::from(state) & u16::from(mods) & !u16::from(self.keyboard.num_lock)
    }

    /// [D3] [R3] Binds the key chords to the keycodes of the current mapping.
    fn grab_keys(&mut self) -> Result<(), Error> {
        self.conn.ungrab_key(Grab::ANY, self.root, ModMask::ANY)?;
        self.keys.clear();
        let locks = self.locks();
        for (mods, key, action) in &self.key_chords {
            let codes = match *key {
                Key::Sym(sym) => self.keyboard.keycodes(sym),
                Key::Code(code) => vec![code],
            };
            for code in codes {
                for &lock in &locks {
                    let mods = *mods | lock;
                    self.conn.grab_key(
                        false,
                        self.root,
                        mods,
                        code,
                        GrabMode::ASYNC,
                        GrabMode::ASYNC,
                    )?;
                }
                self.keys.insert((u16::from(*mods), code), action.clone());
            }
        }
        Ok(())
    }

    /// [F3] [D4] Click-to-focus grabs, then the button chords, which take precedence.
    fn grab_buttons(&self, window: Window) -> Result<(), Error> {
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
        for &(mods, button) in self.buttons.keys() {
            for lock in self.locks() {
                self.conn.grab_button(
                    false,
                    window,
                    EventMask::BUTTON_PRESS,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                    NONE,
                    NONE,
                    ButtonIndex::from(button),
                    ModMask::from(mods) | lock,
                )?;
            }
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
            // [F2] Refocusing the focused window would take the focus from its subwindows.
            Event::EnterNotify(e) => {
                if e.mode == NotifyMode::NORMAL
                    && e.detail != NotifyDetail::INFERIOR
                    && self.focused != Some(e.event)
                    && self.clients.contains_key(&e.event)
                {
                    self.focus(e.event, false)?;
                }
            }
            Event::KeyPress(e) => {
                if let Some(action) = self.keys.get(&(self.clean(e.state), e.detail)).cloned() {
                    self.run_action(action)?;
                }
            }
            Event::ButtonPress(e) => {
                let action = self.buttons.get(&(self.clean(e.state), e.detail)).cloned();
                let managed = self.clients.contains_key(&e.event);
                match action {
                    // [D4] The chord's grab is asynchronous, so the press never reaches
                    // the window.
                    Some(action) if managed => {
                        self.focus(e.event, false)?;
                        self.run_action(action)?;
                    }
                    Some(action) if e.event == self.root && e.child == NONE => {
                        self.run_action(action)?;
                    }
                    // [F3] The grab is synchronous: replay the press so the window gets it.
                    _ => {
                        if managed {
                            self.focus(e.event, true)?;
                        }
                        self.conn.allow_events(Allow::REPLAY_POINTER, e.time)?;
                    }
                }
            }
            // [R3]
            Event::MappingNotify(e) if e.request != Mapping::POINTER => {
                self.keyboard = Keyboard::read(&self.conn)?;
                self.grab_keys()?;
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
        let attributes =
            ChangeWindowAttributesAux::new().event_mask(mask).border_pixel(self.border.normal);
        self.conn.change_window_attributes(window, &attributes)?;
        let geometry = self.conn.get_geometry(window)?;
        let Some(hints) = self.read_hints(window)? else {
            return Ok(());
        };
        let Some(geometry) = vanished(geometry.reply())? else {
            return Ok(());
        };
        debug!("manage {window:#x}");
        let border = ConfigureWindowAux::new().border_width(u32::from(self.border.width));
        self.conn.configure_window(window, &border)?;
        self.grab_buttons(window)?;
        let state = self.atoms.WM_STATE;
        self.conn.change_property32(
            PropMode::REPLACE,
            window,
            state,
            state,
            &[NORMAL_STATE, NONE],
        )?;
        self.clients.insert(window, Client { hints, border_width: geometry.border_width });
        self.order.push(window);
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
        self.order.retain(|&w| w != window);
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
        if !hints.focusable() {
            return Ok(());
        }
        // A click on the focused window still sets the focus: a client may have moved it.
        if let Some(previous) = self.focused.filter(|&w| w != window) {
            let normal = ChangeWindowAttributesAux::new().border_pixel(self.border.normal);
            self.conn.change_window_attributes(previous, &normal)?;
        }
        if hints.accepts_input {
            self.conn.set_input_focus(InputFocus::POINTER_ROOT, window, CURRENT_TIME)?;
        }
        if hints.take_focus {
            self.send_protocol(window, self.atoms.WM_TAKE_FOCUS)?;
        }
        let focused = ChangeWindowAttributesAux::new().border_pixel(self.border.focused);
        self.conn.change_window_attributes(window, &focused)?;
        self.history.retain(|&w| w != window);
        self.history.insert(0, window);
        self.focused = Some(window);
        Ok(())
    }

    /// [C2] Gives every managed window its original border back before sugoi exits.
    fn release_all(&mut self) -> Result<(), Error> {
        for (&window, client) in &self.clients {
            let border = ConfigureWindowAux::new().border_width(u32::from(client.border_width));
            self.conn.configure_window(window, &border)?;
        }
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
        // A malformed WM_HINTS reads as absent; x11rb reports it as a connection error.
        let wm_hints = match wm_hints.reply() {
            Err(ReplyError::ConnectionError(ConnectionError::ParseError(e))) => {
                debug!("malformed WM_HINTS on {window:#x}: {e:?}");
                None
            }
            reply => match vanished(reply)? {
                Some(wm_hints) => wm_hints,
                None => return Ok(None),
            },
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

pub(crate) fn lost(e: ReplyError) -> Error {
    Error::Connection(e.to_string())
}

/// Logs a config error and shows it through `notify-send`.
fn report(message: &str) {
    log::error!("{message}");
    notify(message);
}

/// [D5] Replaces the process with sugoi started the same way; returns if that fails.
fn restart() {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_else(|| "sugoi".into());
    info!("restart");
    let error = std::process::Command::new(&program).args(args).exec();
    log::error!("restart failed: {error}");
}
