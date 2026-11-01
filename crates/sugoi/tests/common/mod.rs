//! Test harness: an Xvfb server per test, sugoi in a thread, and test clients that
//! observe the server the way real applications do. Each test file uses part of it.
#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command as Process, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sugoi::{Command, Error, Handle, Layout, Script, Settings, Wm};
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

/// The number of cases a property runs: `base` in the gates, times the positive integer
/// `SUGOI_PROPTEST_SCALE` for a long run.
///
/// # Panics
///
/// When the variable is set to anything but a positive integer, so a mistyped scale
/// never silently runs the base counts.
pub fn cases(base: u32) -> u32 {
    let scale = match std::env::var("SUGOI_PROPTEST_SCALE") {
        Err(std::env::VarError::NotPresent) => 1,
        Ok(value) if value.trim().is_empty() => 1,
        Ok(value) => value.trim().parse::<u32>().ok().filter(|n| *n > 0).unwrap_or_else(|| {
            panic!("SUGOI_PROPTEST_SCALE must be a positive integer, not {value:?}")
        }),
        Err(error) => panic!("SUGOI_PROPTEST_SCALE: {error}"),
    };
    base.saturating_mul(scale)
}

/// A script with fixed settings, without Ruby.
pub struct Fixed(pub Settings);

impl Script for Fixed {
    fn load(&mut self) -> Result<Settings, String> {
        Ok(self.0.clone())
    }

    fn run(&mut self, block: usize) -> Result<Vec<Command>, String> {
        Err(format!("no block {block}"))
    }
}

/// The default settings with only the float layout.
pub fn floating() -> Settings {
    Settings { layouts: vec![Layout::Float], ..Settings::default() }
}

/// Waits up to 3 s for `condition`, then panics with `what`.
pub fn eventually(what: &str, condition: impl FnMut() -> bool) {
    eventually_within(Duration::from_secs(3), what, condition);
}

pub fn eventually_within(limit: Duration, what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for: {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

/// Gives sugoi time to act when a test checks that something does not happen.
pub fn settle() {
    thread::sleep(Duration::from_millis(150));
}

/// Keycodes of Xvfb's default US mapping.
pub mod keycode {
    pub const SUPER: u8 = 133;
    pub const SHIFT: u8 = 50;
    pub const CTRL: u8 = 37;
    pub const ALT: u8 = 64;
    pub const NUM_LOCK: u8 = 77;
    pub const RETURN: u8 = 36;
    pub const J: u8 = 44;
    pub const K: u8 = 45;
    pub const Q: u8 = 24;
    pub const E: u8 = 26;
    pub const R: u8 = 27;
    pub const X: u8 = 53;
    pub const Z: u8 = 52;
    pub const ONE: u8 = 10;
}

/// The sugoi binary running a config, with stub `systemd-run` and `notify-send` that
/// log their arguments, one call per line.
pub struct Binary {
    child: Child,
    pub dir: PathBuf,
    pub config: PathBuf,
}

impl Binary {
    fn start(display: &str, config: Option<&str>) -> Binary {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let id = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("sugoi-test-{}-{id}", std::process::id()));
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        // systemd-run's stub runs `sleep`, so a test can see whose child the app is.
        let exec_sleep = "while [ $# -gt 0 ] && [ \"$1\" != -- ]; do shift; done\n\
                          [ \"$2\" = sleep ] && shift && exec \"$@\"\n";
        let stubs = [("systemd-run", "spawn.log", exec_sleep), ("notify-send", "notify.log", "")];
        for (stub, log, tail) in stubs {
            let log = dir.join(log);
            let script = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n{tail}", log.display());
            let path = bin.join(stub);
            fs::write(&path, script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let config_path = dir.join("config.rb");
        if let Some(config) = config {
            fs::write(&config_path, config).unwrap();
        }
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
        let child = Process::new(env!("CARGO_BIN_EXE_sugoi"))
            .arg("--config")
            .arg(&config_path)
            .env("DISPLAY", display)
            .env("PATH", path)
            .stderr(fs::File::create(dir.join("stderr.log")).unwrap())
            .spawn()
            .unwrap();
        Binary { child, dir, config: config_path }
    }

    /// The lines a stub logged.
    pub fn log(&self, name: &str) -> Vec<String> {
        let text = fs::read_to_string(self.dir.join(name)).unwrap_or_default();
        text.lines().map(str::to_owned).collect()
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn status(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().unwrap()
    }
}

impl Drop for Binary {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if thread::panicking() {
            let log = fs::read_to_string(self.dir.join("stderr.log")).unwrap_or_default();
            eprintln!("sugoi's stderr:\n{log}");
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub struct Session {
    pub x: Xvfb,
    pub conn: RustConnection,
    pub root: Window,
    wm: Option<(Handle, JoinHandle<Result<(), Error>>)>,
    pub binary: Option<Binary>,
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
        Session { x, conn, root, wm: None, binary: None }
    }

    /// Starts sugoi in the float layout, where windows keep the geometry tests give them.
    pub fn start_wm(&mut self) {
        self.start_wm_with(floating());
    }

    /// An X server with sugoi running `settings`.
    pub fn with_settings(settings: Settings) -> Session {
        let mut session = Session::without_wm();
        session.start_wm_with(settings);
        session
    }

    pub fn start_wm_with(&mut self, settings: Settings) {
        let display = self.x.display.clone();
        let (sender, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let wm = Wm::new(Some(&display), Box::new(Fixed(settings)))?;
            sender.send(wm.handle()).unwrap();
            wm.run()
        });
        let handle = receiver.recv_timeout(Duration::from_secs(5)).expect("sugoi failed to start");
        self.wm = Some((handle, thread));
    }

    /// An X server with the sugoi binary running `config`, ready for input.
    pub fn with_config(config: &str) -> Session {
        Session::with_binary(Some(config))
    }

    /// The sugoi binary without a config file.
    pub fn without_config() -> Session {
        Session::with_binary(None)
    }

    /// The sugoi binary running `config`, without waiting for it to be ready.
    pub fn starting(config: &str) -> Session {
        let mut session = Session::without_wm();
        session.binary = Some(Binary::start(&session.x.display, Some(config)));
        session
    }

    fn with_binary(config: Option<&str>) -> Session {
        let mut session = Session::without_wm();
        session.binary = Some(Binary::start(&session.x.display, config));
        let redirect = EventMask::SUBSTRUCTURE_REDIRECT;
        eventually("sugoi is the WM", || {
            let attributes = session.conn.get_window_attributes(session.root);
            let masks = attributes.unwrap().reply().unwrap().all_event_masks;
            masks & redirect == redirect
        });
        session.sync();
        session
    }

    pub fn binary(&mut self) -> &mut Binary {
        self.binary.as_mut().unwrap()
    }

    /// Presses `keys` in order, then releases them in reverse.
    pub fn keys(&self, keys: &[u8]) {
        for &key in keys {
            self.conn.xtest_fake_input(KEY_PRESS_EVENT, key, 0, self.root, 0, 0, 0).unwrap();
        }
        for &key in keys.iter().rev() {
            self.conn.xtest_fake_input(KEY_RELEASE_EVENT, key, 0, self.root, 0, 0, 0).unwrap();
        }
        self.conn.sync().unwrap();
    }

    pub fn send(&self, command: Command) -> bool {
        self.wm.as_ref().unwrap().0.send(command)
    }

    /// Waits for sugoi's thread to end and returns its result.
    pub fn join(&mut self) -> Result<(), Error> {
        let (_, thread) = self.wm.take().unwrap();
        thread.join().unwrap()
    }

    pub fn wm_running(&mut self) -> bool {
        if let Some(binary) = self.binary.as_mut() {
            return binary.status().is_none();
        }
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
        // Closing the marker's connection destroys it; wait for that, since in a tiling
        // layout the marker covers the screen.
        let window = marker.window;
        drop(marker);
        eventually("marker gone", || !self.exists(window));
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

    /// The window's outer rectangle: x, y, and size with the border.
    pub fn outer(&self, window: Window) -> (i32, i32, u32, u32) {
        let g = self.geometry(window).unwrap();
        let border = 2 * u32::from(g.border_width);
        (g.x.into(), g.y.into(), u32::from(g.width) + border, u32::from(g.height) + border)
    }

    /// Drags with `button` and `key` held, from `from` to `to`, pausing after the press so
    /// sugoi can grab the pointer.
    pub fn drag(&self, key: u8, button: u8, from: (i16, i16), to: (i16, i16)) {
        let fake = |kind, detail, x, y| {
            self.conn.xtest_fake_input(kind, detail, 0, self.root, x, y, 0).unwrap();
            self.conn.sync().unwrap();
        };
        fake(MOTION_NOTIFY_EVENT, 0, from.0, from.1);
        fake(KEY_PRESS_EVENT, key, 0, 0);
        fake(BUTTON_PRESS_EVENT, button, 0, 0);
        settle();
        fake(MOTION_NOTIFY_EVENT, 0, to.0, to.1);
        settle();
        fake(BUTTON_RELEASE_EVENT, button, 0, 0);
        fake(KEY_RELEASE_EVENT, key, 0, 0);
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
    /// Whether `WM_NORMAL_HINTS` gives the position as the user's ([W2]), so a floating
    /// window stays where the test puts it.
    pub positioned: bool,
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
            positioned: true,
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
        if spec.positioned {
            // USPosition and USSize, then x, y, width, height; the other 13 fields unset.
            let mut hints = [0u32; 18];
            hints[..5].copy_from_slice(&[
                1 | 2,
                spec.x as u32,
                spec.y as u32,
                u32::from(spec.width),
                u32::from(spec.height),
            ]);
            conn.change_property32(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_NORMAL_HINTS,
                AtomEnum::WM_SIZE_HINTS,
                &hints,
            )
            .unwrap();
        }
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
