//! Test harness: an Xvfb server per test, sugoi in a thread, and test clients that
//! observe the server the way real applications do.

use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command as Process, Stdio};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sugoi::{Command, Config, Error, Handle, Wm};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub const FOCUSED: u32 = 0xaa007b;
pub const NORMAL: u32 = 0x000000;
pub const POINTER_ROOT: Window = 1;

pub struct Xvfb {
    child: Child,
    _stdout: BufReader<ChildStdout>,
    pub display: String,
}

impl Xvfb {
    pub fn start() -> Xvfb {
        let mut child = Process::new("Xvfb")
            .args(["-displayfd", "1", "-screen", "0", "1280x800x24", "-nolisten", "tcp"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Xvfb not found: install xorg-server-xvfb");
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        Xvfb { child, _stdout: stdout, display: format!(":{}", line.trim()) }
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Waits up to 3 s for `condition`, then panics with `what`.
pub fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for: {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

/// Gives sugoi time to act when a test checks that something does not happen.
pub fn settle() {
    thread::sleep(Duration::from_millis(150));
}

pub struct Session {
    pub x: Xvfb,
    pub conn: RustConnection,
    pub root: Window,
    wm: Option<(Handle, JoinHandle<Result<(), Error>>)>,
}

impl Session {
    /// An X server with sugoi running.
    pub fn new() -> Session {
        let mut session = Session::without_wm();
        session.start_wm();
        session
    }

    /// An X server without a WM, for windows that exist before sugoi starts.
    pub fn without_wm() -> Session {
        let _ = env_logger::builder().is_test(true).try_init();
        let x = Xvfb::start();
        let (conn, screen) = x11rb::connect(Some(&x.display)).unwrap();
        let root = conn.setup().roots[screen].root;
        Session { x, conn, root, wm: None }
    }

    pub fn start_wm(&mut self) {
        let display = self.x.display.clone();
        let (sender, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let wm = Wm::new(Some(&display), Config::default())?;
            sender.send(wm.handle()).unwrap();
            wm.run()
        });
        let handle = receiver.recv_timeout(Duration::from_secs(5)).expect("sugoi failed to start");
        self.wm = Some((handle, thread));
    }

    pub fn send(&self, command: Command) -> bool {
        self.wm.as_ref().unwrap().0.send(command)
    }

    /// Waits for sugoi's thread to end and returns its result.
    pub fn join(&mut self) -> Result<(), Error> {
        let (_, thread) = self.wm.take().unwrap();
        thread.join().unwrap()
    }

    pub fn wm_running(&self) -> bool {
        self.wm.as_ref().is_some_and(|(_, thread)| !thread.is_finished())
    }

    pub fn atom(&self, name: &str) -> Atom {
        self.conn.intern_atom(false, name.as_bytes()).unwrap().reply().unwrap().atom
    }

    pub fn app(&self, spec: AppSpec) -> App {
        App::new(&self.x.display, spec)
    }

    /// Maps a plain window and waits until sugoi manages it: everything sugoi received
    /// before has been handled.
    pub fn sync(&self) {
        let marker = self.app(AppSpec::at(1100, 700, 20, 20));
        marker.map();
        eventually("marker managed", || self.wm_state(marker.window).is_some());
    }

    pub fn focus(&self) -> Window {
        self.conn.get_input_focus().unwrap().reply().unwrap().focus
    }

    pub fn wm_state(&self, window: Window) -> Option<Vec<u32>> {
        let state = self.atom("WM_STATE");
        let reply = self.conn.get_property(false, window, state, state, 0, 2).ok()?.reply().ok()?;
        reply.value32().map(|v| v.collect()).filter(|v: &Vec<u32>| !v.is_empty())
    }

    pub fn geometry(&self, window: Window) -> Option<GetGeometryReply> {
        self.conn.get_geometry(window).ok()?.reply().ok()
    }

    pub fn exists(&self, window: Window) -> bool {
        self.geometry(window).is_some()
    }

    pub fn viewable(&self, window: Window) -> bool {
        let attributes = self.conn.get_window_attributes(window).unwrap().reply().unwrap();
        attributes.map_state == MapState::VIEWABLE
    }

    /// The color of the window's border, read from the screen at its outer corner.
    pub fn border_color(&self, window: Window) -> u32 {
        let g = self.geometry(window).unwrap();
        let image =
            self.conn.get_image(ImageFormat::Z_PIXMAP, self.root, g.x, g.y, 1, 1, !0).unwrap();
        let data = image.reply().unwrap().data;
        u32::from_le_bytes([data[0], data[1], data[2], 0])
    }

    /// Whether `a` is stacked above `b`.
    pub fn above(&self, a: Window, b: Window) -> bool {
        let children = self.conn.query_tree(self.root).unwrap().reply().unwrap().children;
        let index = |w| children.iter().position(|&c| c == w).unwrap();
        index(a) > index(b)
    }

    pub fn move_pointer(&self, x: i16, y: i16) {
        self.conn.xtest_fake_input(MOTION_NOTIFY_EVENT, 0, 0, self.root, x, y, 0).unwrap();
        self.conn.sync().unwrap();
    }

    pub fn click(&self, x: i16, y: i16) {
        self.press(x, y, 1);
    }

    /// Presses and releases `button` at (x, y); 4 and 5 are the wheel.
    pub fn press(&self, x: i16, y: i16, button: u8) {
        self.move_pointer(x, y);
        self.conn.xtest_fake_input(BUTTON_PRESS_EVENT, button, 0, self.root, 0, 0, 0).unwrap();
        self.conn.xtest_fake_input(BUTTON_RELEASE_EVENT, button, 0, self.root, 0, 0, 0).unwrap();
        self.conn.sync().unwrap();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some((handle, thread)) = self.wm.take() {
            handle.send(Command::Quit);
            if let Ok(Err(e)) = thread.join() {
                eprintln!("sugoi ended with an error: {e}");
            }
        }
    }
}

/// How a test window is created.
#[derive(Clone)]
pub struct AppSpec {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub override_redirect: bool,
    /// `WM_HINTS` input flag; `None` leaves `WM_HINTS` unset.
    pub input: Option<bool>,
    pub protocols: Vec<&'static str>,
    pub events: EventMask,
}

impl AppSpec {
    pub fn at(x: i16, y: i16, width: u16, height: u16) -> AppSpec {
        AppSpec {
            x,
            y,
            width,
            height,
            override_redirect: false,
            input: None,
            protocols: Vec::new(),
            events: EventMask::NO_EVENT,
        }
    }
}

/// A client with its own connection and one top-level window.
pub struct App {
    pub conn: RustConnection,
    pub window: Window,
}

impl App {
    pub fn new(display: &str, spec: AppSpec) -> App {
        let (conn, screen) = x11rb::connect(Some(display)).unwrap();
        let root = conn.setup().roots[screen].root;
        let window = conn.generate_id().unwrap();
        let attributes = CreateWindowAux::new()
            .background_pixel(0xffffff)
            .override_redirect(u32::from(spec.override_redirect))
            .event_mask(spec.events);
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            spec.x,
            spec.y,
            spec.width,
            spec.height,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &attributes,
        )
        .unwrap();
        if let Some(input) = spec.input {
            // WM_HINTS with only the input flag set.
            let hints = [1, u32::from(input), 0, 0, 0, 0, 0, 0, 0];
            conn.change_property32(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_HINTS,
                AtomEnum::WM_HINTS,
                &hints,
            )
            .unwrap();
        }
        if !spec.protocols.is_empty() {
            let atoms: Vec<Atom> = spec.protocols.iter().map(|p| atom(&conn, p)).collect();
            let protocols = atom(&conn, "WM_PROTOCOLS");
            conn.change_property32(PropMode::REPLACE, window, protocols, AtomEnum::ATOM, &atoms)
                .unwrap();
        }
        conn.sync().unwrap();
        App { conn, window }
    }

    pub fn map(&self) {
        self.conn.map_window(self.window).unwrap();
        self.conn.sync().unwrap();
    }

    pub fn atom(&self, name: &str) -> Atom {
        atom(&self.conn, name)
    }

    /// Waits up to 3 s for an event matching `matches`.
    pub fn expect_event(&self, what: &str, mut matches: impl FnMut(&Event) -> bool) -> Event {
        let mut found = None;
        eventually(what, || {
            while let Some(event) = self.conn.poll_for_event().unwrap() {
                if matches(&event) {
                    found = Some(event);
                    return true;
                }
            }
            false
        });
        found.unwrap()
    }

    /// Whether a `WM_PROTOCOLS` message for `protocol` arrives within 3 s.
    pub fn expect_protocol(&self, protocol: &str) {
        let (protocols, wanted) = (self.atom("WM_PROTOCOLS"), self.atom(protocol));
        self.expect_event(protocol, |e| {
            matches!(e, Event::ClientMessage(m)
                if m.type_ == protocols && m.data.as_data32()[0] == wanted)
        });
    }
}

fn atom(conn: &RustConnection, name: &str) -> Atom {
    conn.intern_atom(false, name.as_bytes()).unwrap().reply().unwrap().atom
}
