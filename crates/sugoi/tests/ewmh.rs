//! Contract cases for the EWMH part of `doc/spec/ewmh.md`, one behavior per case.
//! Requests are sent as bars, pagers and apps send them: client messages to the root.

mod common;

use common::*;
use sugoi::{Command, Goto, Settings, State};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;

fn session() -> Session {
    Session::with_settings(Settings::default())
}

fn app(session: &Session) -> App {
    let spec = AppSpec { protocols: vec!["WM_DELETE_WINDOW"], ..AppSpec::at(0, 0, 200, 100) };
    let app = session.app(spec);
    app.map();
    eventually("focused", || session.focus() == app.window);
    app
}

fn values(session: &Session, window: Window, name: &str) -> Vec<u32> {
    let atom = session.atom(name);
    let any: Atom = AtomEnum::ANY.into();
    let reply = session.conn.get_property(false, window, atom, any, 0, 1024).unwrap();
    reply.reply().unwrap().value32().map(|v| v.collect()).unwrap_or_default()
}

fn text(session: &Session, window: Window, name: &str) -> String {
    let atom = session.atom(name);
    let any: Atom = AtomEnum::ANY.into();
    let reply = session.conn.get_property(false, window, atom, any, 0, 1024).unwrap();
    String::from_utf8(reply.reply().unwrap().value).unwrap()
}

/// Sends a client message to the root, as EWMH asks.
fn message(session: &Session, window: Window, kind: &str, data: [u32; 5]) {
    let event = ClientMessageEvent::new(32, window, session.atom(kind), data);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    session.conn.send_event(false, session.root, mask, event).unwrap();
    session.conn.sync().unwrap();
}

fn states(session: &Session, window: Window) -> Vec<u32> {
    values(session, window, "_NET_WM_STATE")
}

/// [E1]
#[test]
fn e1_sugoi_names_itself_and_what_it_supports() {
    let session = session();
    let check = values(&session, session.root, "_NET_SUPPORTING_WM_CHECK");
    assert_eq!(check.len(), 1);
    assert_eq!(values(&session, check[0], "_NET_SUPPORTING_WM_CHECK"), check);
    assert_eq!(text(&session, check[0], "_NET_WM_NAME"), "sugoi");
    let supported = values(&session, session.root, "_NET_SUPPORTED");
    for name in
        ["_NET_WM_STATE_FULLSCREEN", "_NET_WORKAREA", "_NET_CLOSE_WINDOW", "_NET_DESKTOP_NAMES"]
    {
        assert!(supported.contains(&session.atom(name)), "{name}");
    }
}

/// [E2]
#[test]
fn e2_client_lists_and_the_active_window_follow_changes() {
    let session = session();
    let a = app(&session);
    let b = app(&session);
    eventually("lists", || {
        values(&session, session.root, "_NET_CLIENT_LIST") == [a.window, b.window]
            && values(&session, session.root, "_NET_ACTIVE_WINDOW") == [b.window]
    });
    assert_eq!(values(&session, session.root, "_NET_CLIENT_LIST_STACKING"), [a.window, b.window]);
    b.conn.destroy_window(b.window).unwrap();
    b.conn.flush().unwrap();
    eventually("b gone", || {
        values(&session, session.root, "_NET_CLIENT_LIST") == [a.window]
            && values(&session, session.root, "_NET_ACTIVE_WINDOW") == [a.window]
    });
}

/// [E3]
#[test]
fn e3_thirty_named_workspaces_on_one_screen() {
    let session = session();
    assert_eq!(values(&session, session.root, "_NET_NUMBER_OF_DESKTOPS"), [30]);
    let names: Vec<String> = (1..=30).map(|n| format!("{n}\0")).collect();
    assert_eq!(text(&session, session.root, "_NET_DESKTOP_NAMES"), names.concat());
    assert_eq!(values(&session, session.root, "_NET_DESKTOP_GEOMETRY"), [1280, 800]);
    assert_eq!(values(&session, session.root, "_NET_DESKTOP_VIEWPORT"), vec![0; 60]);
}

/// [E4]
#[test]
fn e4_a_dock_reserves_its_strut_and_is_not_managed() {
    let session = session();
    let a = app(&session);
    let bar = session.app(AppSpec {
        kind: Some("_NET_WM_WINDOW_TYPE_DOCK"),
        positioned: false,
        ..AppSpec::at(0, 0, 1280, 30)
    });
    let strut = [0, 0, 30, 0, 0, 0, 0, 0, 0, 1279, 0, 0];
    let partial = session.atom("_NET_WM_STRUT_PARTIAL");
    bar.conn
        .change_property32(PropMode::REPLACE, bar.window, partial, AtomEnum::CARDINAL, &strut)
        .unwrap();
    bar.map();
    eventually("bar shown", || session.viewable(bar.window));
    eventually("a below the bar", || session.outer(a.window) == (0, 30, 1280, 770));
    assert_eq!(session.wm_state(bar.window), None);
    assert_eq!(session.geometry(bar.window).unwrap().border_width, 0);
    assert_eq!(session.focus(), a.window);
    let area = values(&session, session.root, "_NET_WORKAREA");
    assert_eq!((area.len(), &area[..4]), (120, &[0, 30, 1280, 770][..]));
    bar.conn.unmap_window(bar.window).unwrap();
    bar.conn.flush().unwrap();
    eventually("the screen again", || session.outer(a.window) == (0, 0, 1280, 800));
}

/// [E5] [E6]
#[test]
fn e5_window_states_and_allowed_actions_are_published() {
    let session = session();
    let a = app(&session);
    let atom = |name| session.atom(name);
    assert!(
        values(&session, a.window, "_NET_WM_ALLOWED_ACTIONS")
            .contains(&atom("_NET_WM_ACTION_CLOSE"))
    );
    assert!(session.send(Command::Toggle(State::Fullscreen)));
    eventually("fullscreen", || states(&session, a.window) == [atom("_NET_WM_STATE_FULLSCREEN")]);
    assert!(session.send(Command::Toggle(State::Fullscreen)));
    assert!(session.send(Command::Toggle(State::Maximized)));
    let maximized =
        vec![atom("_NET_WM_STATE_MAXIMIZED_VERT"), atom("_NET_WM_STATE_MAXIMIZED_HORZ")];
    eventually("maximized", || states(&session, a.window) == maximized);
    assert!(session.send(Command::Workspace(Goto::Number(2))));
    eventually("hidden", || states(&session, a.window).contains(&atom("_NET_WM_STATE_HIDDEN")));
}

/// [E7] An app's fullscreen, as F11 asks for it.
#[test]
fn e7_an_app_asks_for_fullscreen() {
    let session = session();
    let a = app(&session);
    let fullscreen = session.atom("_NET_WM_STATE_FULLSCREEN");
    message(&session, a.window, "_NET_WM_STATE", [1, fullscreen, 0, 1, 0]);
    eventually("fullscreen", || session.geometry(a.window).unwrap().border_width == 0);
    message(&session, a.window, "_NET_WM_STATE", [2, fullscreen, 0, 1, 0]);
    // Fullscreen and tiled alone both cover the screen: the border tells them apart.
    eventually("back", || session.geometry(a.window).unwrap().border_width == 3);
    assert_eq!(session.outer(a.window), (0, 0, 1280, 800));
}

/// [E8]
#[test]
fn e8_activation_finds_a_window_wherever_it_is() {
    let session = session();
    let a = app(&session);
    assert!(session.send(Command::Minimize));
    eventually("minimized", || !session.viewable(a.window));
    assert!(session.send(Command::Workspace(Goto::Number(5))));
    let _b = app(&session);
    message(&session, a.window, "_NET_ACTIVE_WINDOW", [2, 0, 0, 0, 0]);
    eventually("a shown and focused", || session.viewable(a.window) && session.focus() == a.window);
    assert_eq!(values(&session, session.root, "_NET_CURRENT_DESKTOP"), [0]);
}

/// [E9]
#[test]
fn e9_a_bar_switches_workspaces_and_moves_windows() {
    let session = session();
    let a = app(&session);
    message(&session, session.root, "_NET_CURRENT_DESKTOP", [2, 0, 0, 0, 0]);
    eventually("on 3, not cycled", || {
        values(&session, session.root, "_NET_CURRENT_DESKTOP") == [2]
    });
    message(&session, session.root, "_NET_CURRENT_DESKTOP", [2, 0, 0, 0, 0]);
    settle();
    assert_eq!(values(&session, session.root, "_NET_CURRENT_DESKTOP"), [2]);
    message(&session, a.window, "_NET_WM_DESKTOP", [2, 1, 0, 0, 0]);
    eventually("a moved here", || session.viewable(a.window));
    assert_eq!(values(&session, a.window, "_NET_WM_DESKTOP"), [2]);
}

/// [E10]
#[test]
fn e10_a_pager_closes_a_window() {
    let session = session();
    let a = app(&session);
    message(&session, a.window, "_NET_CLOSE_WINDOW", [0, 2, 0, 0, 0]);
    a.expect_protocol("WM_DELETE_WINDOW");
}

/// A dock along the top, `strut` pixels high, mapped.
fn dock(session: &Session, strut: u32) -> App {
    let bar = session.app(AppSpec {
        kind: Some("_NET_WM_WINDOW_TYPE_DOCK"),
        positioned: false,
        ..AppSpec::at(0, 0, 1280, 30)
    });
    let atom = session.atom("_NET_WM_STRUT");
    bar.conn
        .change_property32(
            PropMode::REPLACE,
            bar.window,
            atom,
            AtomEnum::CARDINAL,
            &[0, 0, strut, 0],
        )
        .unwrap();
    bar.map();
    eventually("bar shown", || session.viewable(bar.window));
    bar
}

/// [E4] A fullscreen window covers the bar; others stay below it.
#[test]
fn e4_fullscreen_goes_above_docks() {
    let session = session();
    let a = app(&session);
    let bar = dock(&session, 30);
    eventually("bar above a", || session.above(bar.window, a.window));
    assert!(session.send(Command::Toggle(State::Fullscreen)));
    eventually("a above the bar", || session.above(a.window, bar.window));
}

/// [E4] A strut past the screen breaks nothing.
#[test]
fn e4_a_huge_strut_leaves_a_pixel() {
    let mut session = session();
    let a = app(&session);
    let _bar = dock(&session, u32::MAX);
    // The work area keeps the last row; the window, at least 1 px, is there.
    eventually("a squeezed", || session.outer(a.window).1 == 799);
    assert!(session.wm_running());
}

/// [E5] A state the app sets before mapping counts.
#[test]
fn e5_an_app_can_start_fullscreen() {
    let session = session();
    let video = session.app(AppSpec::at(0, 0, 200, 100));
    let (state, fullscreen) =
        (session.atom("_NET_WM_STATE"), session.atom("_NET_WM_STATE_FULLSCREEN"));
    video
        .conn
        .change_property32(PropMode::REPLACE, video.window, state, AtomEnum::ATOM, &[fullscreen])
        .unwrap();
    video.map();
    // Unmanaged, the window has no border either: wait until sugoi has it.
    eventually("managed", || session.wm_state(video.window).is_some());
    settle();
    let g = session.geometry(video.window).unwrap();
    assert_eq!((g.width, g.height, g.border_width), (1280, 800, 0));
    assert_eq!(states(&session, video.window), [fullscreen]);
}

/// [E8] An app's own activation does not pull the user to another workspace.
#[test]
fn e8_an_app_cannot_switch_the_workspace() {
    let session = session();
    let a = app(&session);
    assert!(session.send(Command::Workspace(Goto::Number(4))));
    eventually("a hidden", || !session.viewable(a.window));
    message(&session, a.window, "_NET_ACTIVE_WINDOW", [1, 0, 0, 0, 0]);
    settle();
    assert_eq!(values(&session, session.root, "_NET_CURRENT_DESKTOP"), [3]);
    assert!(!session.viewable(a.window));
}

/// [E4] A dock mapped later, without a new strut, still goes below fullscreen.
#[test]
fn e4_a_dock_mapped_later_stays_below_fullscreen() {
    let session = session();
    let a = app(&session);
    assert!(session.send(Command::Toggle(State::Fullscreen)));
    eventually("fullscreen", || session.geometry(a.window).unwrap().border_width == 0);
    let bar = dock(&session, 0);
    eventually("a above the bar", || session.above(a.window, bar.window));
}

/// [E8] An app may still bring back its own minimized window, here.
#[test]
fn e8_an_app_restores_its_window_on_this_workspace() {
    let session = session();
    let a = app(&session);
    assert!(session.send(Command::Minimize));
    eventually("minimized", || !session.viewable(a.window));
    message(&session, a.window, "_NET_ACTIVE_WINDOW", [1, 0, 0, 0, 0]);
    eventually("restored and focused", || {
        session.viewable(a.window) && session.focus() == a.window
    });
}
