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
    // b is created first, so it starts below a.
    let b = session.app(AppSpec::at(100, 100, 300, 200));
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    b.map();
    eventually("b focused", || session.focus() == b.window);
    eventually("b raised", || session.above(b.window, a.window));
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

/// [F2]
#[test]
fn f2_grab_crossings_do_not_move_the_focus() {
    let session = Session::new();
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    let b = session.app(AppSpec::at(400, 0, 300, 200));
    a.map();
    session.move_pointer(50, 50);
    eventually("a focused", || session.focus() == a.window);
    b.map();
    eventually("b focused", || session.focus() == b.window);
    // b grabs the pointer, as an open menu does, while the pointer rests over a: a
    // gets crossing events in grab and ungrab mode.
    let grab = b.conn.grab_pointer(
        false,
        b.window,
        EventMask::BUTTON_PRESS,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
        x11rb::NONE,
        x11rb::NONE,
        x11rb::CURRENT_TIME,
    );
    assert_eq!(grab.unwrap().reply().unwrap().status, GrabStatus::SUCCESS);
    b.conn.ungrab_pointer(x11rb::CURRENT_TIME).unwrap();
    b.conn.sync().unwrap();
    settle();
    assert_eq!(session.focus(), b.window);
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

/// [S2] and [F5]
#[test]
fn s2_adopted_windows_enter_the_history_topmost_first() {
    let mut session = Session::without_wm();
    let apps: Vec<App> =
        [0, 300, 600].into_iter().map(|x| session.app(AppSpec::at(x, 0, 200, 200))).collect();
    for app in &apps {
        app.map();
    }
    session.move_pointer(350, 50);
    session.start_wm();
    eventually("b focused", || session.focus() == apps[1].window);
    apps[1].conn.destroy_window(apps[1].window).unwrap();
    apps[1].conn.sync().unwrap();
    eventually("topmost c focused", || session.focus() == apps[2].window);
}

/// [M4]
#[test]
fn m4_client_unmap_restores_the_border_and_releases_the_window() {
    let session = Session::new();
    let app = session.app(AppSpec::at(0, 0, 100, 100));
    app.map();
    eventually("managed", || session.wm_state(app.window).is_some());
    assert_eq!(session.geometry(app.window).unwrap().border_width, 3);
    app.conn.unmap_window(app.window).unwrap();
    app.conn.sync().unwrap();
    eventually("WM_STATE deleted", || session.wm_state(app.window).is_none());
    eventually("border restored", || session.geometry(app.window).unwrap().border_width == 0);
    let attributes = session.conn.get_window_attributes(app.window).unwrap().reply().unwrap();
    assert_eq!(attributes.all_event_masks, EventMask::NO_EVENT);
}

/// [F1] and [F4]
#[test]
fn f1_windows_that_cannot_take_focus_are_still_raised() {
    let session = Session::new();
    let inert = session.app(AppSpec { input: Some(false), ..AppSpec::at(100, 100, 300, 200) });
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    inert.map();
    eventually("inert raised", || session.above(inert.window, a.window));
    assert_eq!(session.focus(), a.window);
}

/// [F2]
#[test]
fn f2_reentering_the_focused_window_keeps_the_focus_on_its_subwindow() {
    let session = Session::new();
    let app = session.app(AppSpec::at(0, 0, 300, 200));
    app.map();
    eventually("focused", || session.focus() == app.window);
    let child = app.conn.generate_id().unwrap();
    let aux = CreateWindowAux::new();
    app.conn
        .create_window(0, child, app.window, 10, 10, 50, 50, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
        .unwrap();
    app.conn.map_window(child).unwrap();
    app.conn.set_input_focus(InputFocus::PARENT, child, x11rb::CURRENT_TIME).unwrap();
    app.conn.sync().unwrap();
    for (x, y) in [(200, 150), (1000, 700), (200, 150)] {
        session.move_pointer(x, y);
    }
    settle();
    assert_eq!(session.focus(), child);
}

/// [F4]
#[test]
fn f4_globally_active_window_takes_the_focus_after_a_later_focus_change() {
    let session = Session::new();
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    let b = session.app(AppSpec::at(400, 0, 300, 200));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    session.move_pointer(50, 50);
    eventually("a focused", || session.focus() == a.window);
    // The server's last focus change moves past every timestamp sugoi has seen.
    std::thread::sleep(std::time::Duration::from_millis(50));
    a.conn.set_input_focus(InputFocus::POINTER_ROOT, a.window, x11rb::CURRENT_TIME).unwrap();
    a.conn.sync().unwrap();
    let spec = AppSpec {
        input: Some(false),
        protocols: vec!["WM_TAKE_FOCUS"],
        ..AppSpec::at(800, 0, 300, 200)
    };
    let global = session.app(spec);
    global.map();
    let protocols = global.atom("WM_PROTOCOLS");
    let message = global.expect_event(
        "WM_TAKE_FOCUS",
        |e| matches!(e, Event::ClientMessage(m) if m.type_ == protocols),
    );
    let Event::ClientMessage(message) = message else { unreachable!() };
    let time = message.data.as_data32()[1];
    global.conn.set_input_focus(InputFocus::POINTER_ROOT, global.window, time).unwrap();
    global.conn.sync().unwrap();
    assert_eq!(session.focus(), global.window);
}
