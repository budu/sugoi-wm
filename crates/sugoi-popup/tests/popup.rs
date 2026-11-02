//! Contract cases for `doc/spec/popup.md`: the `sugoi-popup` binary on its own Xvfb,
//! with a work area and a DPI set as sugoi and the X resources would.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

const SCREEN: (i32, i32) = (1280, 800);
/// The work area's top, as a bar 40 px high leaves it.
const TOP: i32 = 40;

struct Xvfb {
    child: Child,
    _stdout: BufReader<ChildStdout>,
    display: String,
    conn: RustConnection,
    root: Window,
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// An X server whose root has a work area and `Xft.dpi` at `dpi`.
fn xvfb(dpi: u32) -> Xvfb {
    let mut child = Command::new("Xvfb")
        .args(["-displayfd", "1", "-screen", "0", "1280x800x24", "-nolisten", "tcp"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Xvfb not found: install xorg-server-xvfb");
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let display = format!(":{}", line.trim());
    let (conn, screen) = x11rb::connect(Some(&display)).unwrap();
    let root = conn.setup().roots[screen].root;
    let atom = |name: &str| conn.intern_atom(false, name.as_bytes()).unwrap().reply().unwrap().atom;
    let area = [0, TOP as u32, SCREEN.0 as u32, (SCREEN.1 - TOP) as u32];
    conn.change_property32(
        PropMode::REPLACE,
        root,
        atom("_NET_WORKAREA"),
        AtomEnum::CARDINAL,
        &area,
    )
    .unwrap();
    let resources = format!("Xft.dpi:\t{dpi}\n");
    conn.change_property8(
        PropMode::REPLACE,
        root,
        AtomEnum::RESOURCE_MANAGER,
        AtomEnum::STRING,
        resources.as_bytes(),
    )
    .unwrap();
    conn.sync().unwrap();
    Xvfb { child, _stdout: stdout, display, conn, root }
}

impl Xvfb {
    fn pointer(&self, x: i16, y: i16) {
        let motion = MOTION_NOTIFY_EVENT;
        self.conn.xtest_fake_input(motion, 0, 0, self.root, x, y, 0).unwrap();
        self.conn.sync().unwrap();
    }

    fn fake(&self, kind: u8, detail: u8) {
        self.conn.xtest_fake_input(kind, detail, 0, self.root, 0, 0, 0).unwrap();
        self.conn.sync().unwrap();
    }

    /// The popup: a mapped override-redirect child of the root.
    fn popup(&self) -> Option<(i32, i32, i32, i32)> {
        let children = self.conn.query_tree(self.root).unwrap().reply().unwrap().children;
        children.into_iter().find_map(|w| {
            let a = self.conn.get_window_attributes(w).unwrap().reply().ok()?;
            if !a.override_redirect || a.map_state != MapState::VIEWABLE {
                return None;
            }
            let g = self.conn.get_geometry(w).unwrap().reply().ok()?;
            Some((g.x.into(), g.y.into(), g.width.into(), g.height.into()))
        })
    }

    fn pixel(&self, x: i32, y: i32) -> u32 {
        let image =
            self.conn.get_image(ImageFormat::Z_PIXMAP, self.root, x as i16, y as i16, 1, 1, !0);
        let data = image.unwrap().reply().unwrap().data;
        u32::from_le_bytes([data[0], data[1], data[2], 0])
    }

    /// Runs `sugoi-popup` with `input`.
    fn run(&self, input: &str) -> Child {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sugoi-popup"))
            .env("DISPLAY", &self.display)
            .stdin(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        child
    }

    fn shown(&self) -> (i32, i32, i32, i32) {
        let start = Instant::now();
        loop {
            if let Some(popup) = self.popup() {
                return popup;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "no popup");
            sleep(Duration::from_millis(10));
        }
    }
}

fn exit(child: &mut Child) -> ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "still running");
        sleep(Duration::from_millis(10));
    }
}

/// [O1] and [O2]
#[test]
fn o1_o2_a_styled_popup_shows_under_the_pointer_below_the_work_area() {
    let x = xvfb(192);
    x.pointer(600, 10);
    let mut child = x.run("<b>Claude usage</b>\n5-hour   7%\n");
    let (px, py, width, height) = x.shown();
    assert_eq!(py, TOP + 10, "5 px at 192 DPI");
    assert!((px + width / 2 - 600).abs() <= 1, "centered on the pointer");
    // Margins and border scale too: 2 px of border, 24 px of margin.
    assert!(width > 2 * 26 && height > 2 * 26);
    assert_eq!(x.pixel(px, py), 0xaa007b, "the border");
    assert_eq!(x.pixel(px + 1, py + 1), 0xaa007b, "2 px of border");
    assert_eq!(x.pixel(px + 2, py + 2), 0x222222, "the background");
    x.pointer(100, 400);
    x.fake(BUTTON_PRESS_EVENT, 1);
    x.fake(BUTTON_RELEASE_EVENT, 1);
    assert!(exit(&mut child).success());
}

/// [O1]
#[test]
fn o1_text_grows_with_the_dpi() {
    let size = |dpi| {
        let x = xvfb(dpi);
        x.pointer(600, 10);
        let mut child = x.run("some text\n");
        let (_, _, width, height) = x.shown();
        x.fake(KEY_PRESS_EVENT, 9);
        x.fake(KEY_RELEASE_EVENT, 9);
        exit(&mut child);
        (width, height)
    };
    let (small, large) = (size(96), size(192));
    assert!(large.0 > small.0 * 3 / 2 && large.1 > small.1 * 3 / 2, "{small:?} {large:?}");
}

/// [O1]
#[test]
fn o1_markup_that_does_not_parse_shows_as_text_and_empty_input_shows_nothing() {
    let x = xvfb(96);
    let size = |input: &str| {
        let mut child = x.run(input);
        let (_, _, width, height) = x.shown();
        x.fake(KEY_PRESS_EVENT, 9);
        x.fake(KEY_RELEASE_EVENT, 9);
        assert!(exit(&mut child).success());
        (width, height)
    };
    assert_eq!(size("<b>unclosed & raw\n"), size("&lt;b&gt;unclosed &amp; raw\n"));
    let mut child = x.run("\n  \n");
    assert!(exit(&mut child).success());
    assert_eq!(x.popup(), None);
}

/// [O2]
#[test]
fn o2_the_popup_stays_on_screen() {
    let x = xvfb(96);
    x.pointer(1279, 10);
    let mut child = x.run(&"wide ".repeat(20));
    let (px, _, width, _) = x.shown();
    assert_eq!(px + width, SCREEN.0);
    x.fake(KEY_PRESS_EVENT, 9);
    x.fake(KEY_RELEASE_EVENT, 9);
    exit(&mut child);
}

/// [O2]
#[test]
fn o2_a_long_popup_is_cut_to_the_screen() {
    let x = xvfb(96);
    x.pointer(600, 10);
    let mut child = x.run(&"a line of text\n".repeat(400));
    let (px, py, width, height) = x.shown();
    assert_eq!(py, TOP + 5);
    assert_eq!(py + height, SCREEN.1, "down to the screen's bottom");
    let bottom = py + height - 1;
    assert_eq!(x.pixel(px + width / 2, bottom), 0xaa007b, "the border, kept");
    x.fake(KEY_PRESS_EVENT, 9);
    x.fake(KEY_RELEASE_EVENT, 9);
    assert!(exit(&mut child).success());
}

/// [O2]
#[test]
fn o2_a_long_popup_stops_above_a_bottom_bar() {
    let x = xvfb(96);
    let atom = x.conn.intern_atom(false, b"_NET_WORKAREA").unwrap().reply().unwrap().atom;
    let area = [0, TOP as u32, SCREEN.0 as u32, (SCREEN.1 - TOP - 30) as u32];
    x.conn.change_property32(PropMode::REPLACE, x.root, atom, AtomEnum::CARDINAL, &area).unwrap();
    x.conn.sync().unwrap();
    x.pointer(600, 10);
    let mut child = x.run(&"a line of text\n".repeat(400));
    let (_, py, _, height) = x.shown();
    assert_eq!(py + height, SCREEN.1 - 30);
    x.fake(KEY_PRESS_EVENT, 9);
    x.fake(KEY_RELEASE_EVENT, 9);
    assert!(exit(&mut child).success());
}

/// [O2] A stale or absurd work area still keeps the popup on screen.
#[test]
fn o2_a_work_area_past_the_screen_is_kept_to_it() {
    for area in [[0, 0, SCREEN.0 as u32, 1440], [0, 4_000_000_000, 7, 400_000_000]] {
        let x = xvfb(96);
        let atom = x.conn.intern_atom(false, b"_NET_WORKAREA").unwrap().reply().unwrap().atom;
        x.conn
            .change_property32(PropMode::REPLACE, x.root, atom, AtomEnum::CARDINAL, &area)
            .unwrap();
        x.conn.sync().unwrap();
        x.pointer(600, 10);
        let mut child = x.run(&"a line of text\n".repeat(400));
        let (px, py, width, height) = x.shown();
        assert!(px >= 0 && py >= 0, "{area:?}");
        assert!(px + width <= SCREEN.0 && py + height <= SCREEN.1, "{area:?}");
        x.fake(KEY_PRESS_EVENT, 9);
        x.fake(KEY_RELEASE_EVENT, 9);
        assert!(exit(&mut child).success());
    }
}

/// [O2]
#[test]
fn o2_a_popup_stays_right_of_a_side_bar() {
    let x = xvfb(96);
    let atom = x.conn.intern_atom(false, b"_NET_WORKAREA").unwrap().reply().unwrap().atom;
    let area = [40, TOP as u32, SCREEN.0 as u32 - 40, (SCREEN.1 - TOP) as u32];
    x.conn.change_property32(PropMode::REPLACE, x.root, atom, AtomEnum::CARDINAL, &area).unwrap();
    x.conn.sync().unwrap();
    x.pointer(10, 10);
    let mut child = x.run("hello\n");
    let (px, _, _, _) = x.shown();
    assert_eq!(px, 40);
    x.fake(KEY_PRESS_EVENT, 9);
    x.fake(KEY_RELEASE_EVENT, 9);
    assert!(exit(&mut child).success());
}

/// [O3]
#[test]
fn o3_the_closing_click_goes_nowhere_else() {
    let x = xvfb(96);
    let catcher = x.conn.generate_id().unwrap();
    let aux = CreateWindowAux::new().event_mask(EventMask::BUTTON_PRESS);
    x.conn
        .create_window(0, catcher, x.root, 0, 400, 400, 400, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
        .unwrap();
    x.conn.map_window(catcher).unwrap();
    x.conn.sync().unwrap();
    x.pointer(600, 10);
    let mut child = x.run("hello\n");
    x.shown();
    x.pointer(100, 500);
    x.fake(BUTTON_PRESS_EVENT, 1);
    x.fake(BUTTON_RELEASE_EVENT, 1);
    assert!(exit(&mut child).success());
    x.conn.sync().unwrap();
    while let Some(event) = x.conn.poll_for_event().unwrap() {
        assert!(
            !matches!(event, x11rb::protocol::Event::ButtonPress(_)),
            "the window under the click got it"
        );
    }
    assert_eq!(x.popup(), None);
}

/// [O3]
#[test]
fn o3_without_the_pointer_it_exits_1_without_showing() {
    let x = xvfb(96);
    let async_ = GrabMode::ASYNC;
    let mask = EventMask::BUTTON_PRESS;
    let grab = x.conn.grab_pointer(false, x.root, mask, async_, async_, NONE, NONE, CURRENT_TIME);
    assert_eq!(grab.unwrap().reply().unwrap().status, GrabStatus::SUCCESS);
    let mut child = x.run("hello\n");
    let start = Instant::now();
    let status = loop {
        assert_eq!(x.popup(), None, "shown while waiting");
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "still running");
        sleep(Duration::from_millis(10));
    };
    assert_eq!(status.code(), Some(1));
}
