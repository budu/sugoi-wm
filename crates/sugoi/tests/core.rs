//! Contract cases for `doc/spec/core.md`, one behavior per case.

mod common;

use common::*;
use sugoi::{Command, Config, Error, Wm};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;

/// [S1]
#[test]
fn s1_second_wm_exits_with_a_message() {
    let session = Session::new();
    let error = Wm::new(Some(&session.x.display), Config::default()).err().unwrap();
    assert!(matches!(error, Error::AnotherWm(_)));
    let expected = format!("another window manager is running on {}", session.x.display);
    assert_eq!(error.to_string(), expected);
}

/// [S2]
#[test]
fn s2_adopts_viewable_windows_and_focuses_the_one_under_the_pointer() {
    let mut session = Session::without_wm();
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    let b = session.app(AppSpec::at(400, 0, 300, 200));
    let unmapped = session.app(AppSpec::at(0, 300, 300, 200));
    let popup = session.app(AppSpec { override_redirect: true, ..AppSpec::at(400, 300, 50, 50) });
    a.map();
    b.map();
    popup.map();
    session.move_pointer(100, 100);
    session.start_wm();

    for window in [a.window, b.window] {
        assert_eq!(session.wm_state(window), Some(vec![1, 0]));
        assert_eq!(session.geometry(window).unwrap().border_width, 3);
    }
    assert_eq!(session.wm_state(unmapped.window), None);
    assert_eq!(session.wm_state(popup.window), None);
    assert_eq!(session.focus(), a.window);
}

/// [S3]
#[test]
fn s3_connection_loss_ends_the_wm_with_an_error() {
    let mut session = Session::new();
    session.x.kill();
    assert!(matches!(session.join(), Err(Error::Connection(_))));
}

/// [M1]
#[test]
fn m1_map_request_manages_the_window() {
    let session = Session::new();
    let app = session.app(AppSpec::at(10, 10, 300, 200));
    app.map();
    eventually("WM_STATE set", || session.wm_state(app.window) == Some(vec![1, 0]));
    assert!(session.viewable(app.window));
    assert_eq!(session.geometry(app.window).unwrap().border_width, 3);
}

/// [M2]
#[test]
fn m2_override_redirect_windows_are_left_alone() {
    let session = Session::new();
    let normal = session.app(AppSpec::at(0, 0, 300, 200));
    normal.map();
    eventually("normal focused", || session.focus() == normal.window);
    let popup = session.app(AppSpec { override_redirect: true, ..AppSpec::at(50, 50, 80, 40) });
    popup.map();
    session.sync();
    assert!(session.viewable(popup.window));
    assert_eq!(session.wm_state(popup.window), None);
    assert_eq!(session.geometry(popup.window).unwrap().border_width, 0);
}

/// [M3]
#[test]
fn m3_configure_request_applies_geometry_but_not_border_width() {
    let session = Session::new();
    let app = session.app(AppSpec::at(0, 0, 100, 100));
    app.map();
    eventually("managed", || session.wm_state(app.window).is_some());
    let request = ConfigureWindowAux::new().x(100).y(50).width(300).height(200).border_width(10);
    app.conn.configure_window(app.window, &request).unwrap();
    app.conn.sync().unwrap();
    eventually("resized", || session.geometry(app.window).unwrap().width == 300);
    let g = session.geometry(app.window).unwrap();
    assert_eq!((g.x, g.y, g.width, g.height, g.border_width), (100, 50, 300, 200, 3));
}

/// [M3]
#[test]
fn m3_refused_configure_request_gets_a_synthetic_notify() {
    let session = Session::new();
    let events = EventMask::STRUCTURE_NOTIFY;
    let app = session.app(AppSpec { events, ..AppSpec::at(20, 30, 100, 100) });
    app.map();
    eventually("managed", || session.wm_state(app.window).is_some());
    while app.conn.poll_for_event().unwrap().is_some() {}
    let request = ConfigureWindowAux::new().border_width(10);
    app.conn.configure_window(app.window, &request).unwrap();
    app.conn.flush().unwrap();
    let event = app.expect_event("synthetic ConfigureNotify", |e| {
        matches!(e, Event::ConfigureNotify(_)) && e.sent_event()
    });
    let Event::ConfigureNotify(n) = event else { unreachable!() };
    assert_eq!((n.x, n.y, n.width, n.height, n.border_width), (20, 30, 100, 100, 3));
}

/// [M4]
#[test]
fn m4_client_unmap_unmanages_and_deletes_wm_state() {
    let session = Session::new();
    let app = session.app(AppSpec::at(0, 0, 100, 100));
    app.map();
    eventually("managed", || session.wm_state(app.window).is_some());
    app.conn.unmap_window(app.window).unwrap();
    app.conn.sync().unwrap();
    eventually("WM_STATE deleted", || session.wm_state(app.window).is_none());
}

/// [M5]
#[test]
fn m5_vanished_windows_do_not_stop_the_wm() {
    let session = Session::new();
    for _ in 0..20 {
        let app = session.app(AppSpec::at(0, 0, 100, 100));
        app.conn.map_window(app.window).unwrap();
        app.conn.destroy_window(app.window).unwrap();
        app.conn.flush().unwrap();
    }
    session.sync();
    assert!(session.wm_running());
}

/// [F1]
#[test]
fn f1_new_windows_are_focused_and_raised() {
    let session = Session::new();
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    let b = session.app(AppSpec::at(100, 100, 300, 200));
    b.map();
    eventually("b focused", || session.focus() == b.window);
    assert!(session.above(b.window, a.window));
}

/// [F2]
#[test]
fn f2_pointer_entering_a_window_focuses_it_without_raising() {
    let session = Session::new();
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    let b = session.app(AppSpec::at(200, 0, 300, 200));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    session.move_pointer(50, 50);
    eventually("a focused", || session.focus() == a.window);
    assert!(session.above(b.window, a.window));
}

/// [F3]
#[test]
fn f3_click_focuses_raises_and_reaches_the_window() {
    let session = Session::new();
    let events = EventMask::BUTTON_PRESS;
    let a = session.app(AppSpec { events, ..AppSpec::at(0, 0, 300, 200) });
    let b = session.app(AppSpec::at(200, 0, 300, 200));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    session.click(50, 50);
    a.expect_event("button press", |e| matches!(e, Event::ButtonPress(_)));
    eventually("a raised", || session.above(a.window, b.window));
    assert_eq!(session.focus(), a.window);
}

/// [F4]
#[test]
fn f4_window_without_input_or_take_focus_cannot_be_focused() {
    let session = Session::new();
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    let inert = session.app(AppSpec { input: Some(false), ..AppSpec::at(400, 0, 300, 200) });
    inert.map();
    eventually("inert managed", || session.wm_state(inert.window).is_some());
    settle();
    assert_eq!(session.focus(), a.window);
    assert_eq!(session.border_color(a.window), FOCUSED);
}

/// [F4]
#[test]
fn f4_take_focus_windows_get_the_message_instead_of_the_focus() {
    let session = Session::new();
    let spec = AppSpec {
        input: Some(false),
        protocols: vec!["WM_TAKE_FOCUS"],
        ..AppSpec::at(0, 0, 300, 200)
    };
    let app = session.app(spec);
    app.map();
    app.expect_protocol("WM_TAKE_FOCUS");
    assert_ne!(session.focus(), app.window);
    assert_eq!(session.border_color(app.window), FOCUSED);
}

/// [F5]
#[test]
fn f5_unmanaging_the_focused_window_focuses_the_previous_one() {
    let session = Session::new();
    let a = session.app(AppSpec::at(0, 0, 200, 200));
    let b = session.app(AppSpec::at(300, 0, 200, 200));
    let c = session.app(AppSpec::at(600, 0, 200, 200));
    for app in [&b, &c, &a] {
        app.map();
        eventually("focused", || session.focus() == app.window);
    }
    // Focus order is now a, c, b; the pointer stays in an empty area.
    a.conn.destroy_window(a.window).unwrap();
    a.conn.sync().unwrap();
    eventually("c focused", || session.focus() == c.window);
}

/// [F5]
#[test]
fn f5_unmanaging_the_last_window_focuses_the_root() {
    let session = Session::new();
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("focused", || session.focus() == app.window);
    app.conn.destroy_window(app.window).unwrap();
    app.conn.sync().unwrap();
    eventually("root focused", || session.focus() == POINTER_ROOT);
}

/// [F6]
#[test]
fn f6_focused_border_has_the_focused_color() {
    let session = Session::new();
    let a = session.app(AppSpec::at(0, 0, 200, 200));
    let b = session.app(AppSpec::at(300, 0, 200, 200));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    eventually("borders", || {
        session.border_color(b.window) == FOCUSED && session.border_color(a.window) == NORMAL
    });
    session.move_pointer(50, 50);
    eventually("borders swapped", || {
        session.border_color(a.window) == FOCUSED && session.border_color(b.window) == NORMAL
    });
}

/// [K1] and [C1]
#[test]
fn k1_close_sends_wm_delete_window_when_supported() {
    let session = Session::new();
    let spec = AppSpec { protocols: vec!["WM_DELETE_WINDOW"], ..AppSpec::at(0, 0, 200, 200) };
    let app = session.app(spec);
    app.map();
    eventually("focused", || session.focus() == app.window);
    assert!(session.send(Command::CloseFocused));
    app.expect_protocol("WM_DELETE_WINDOW");
    assert!(session.exists(app.window));
}

/// [K1] and [C1]
#[test]
fn k1_close_kills_clients_without_wm_delete_window() {
    let session = Session::new();
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("focused", || session.focus() == app.window);
    assert!(session.send(Command::CloseFocused));
    eventually("window gone", || !session.exists(app.window));
}

/// [C1]
#[test]
fn c1_close_without_focus_does_nothing() {
    let session = Session::new();
    assert!(session.send(Command::CloseFocused));
    session.sync();
    assert!(session.wm_running());
}

/// [C2]
#[test]
fn c2_quit_exits_cleanly_and_leaves_windows_mapped() {
    let mut session = Session::new();
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("managed", || session.wm_state(app.window).is_some());
    assert!(session.send(Command::Quit));
    assert!(session.join().is_ok());
    assert!(session.viewable(app.window));
}
