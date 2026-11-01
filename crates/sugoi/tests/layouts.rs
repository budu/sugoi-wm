//! Contract cases for `doc/spec/layouts.md`, one behavior per case. The screen is
//! 1280×800 and borders are 3 px, so a cell is the window plus 6 px each way.

mod common;

use common::keycode::SUPER;
use common::*;
use sugoi::{Binding, Choice, Command, Layout, Master, Settings, Side, State};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::wrapper::ConnectionExt as _;

fn tiling() -> Session {
    Session::with_settings(Settings::default())
}

/// Maps windows one after the other, each focused once it is managed.
fn map(session: &Session, count: usize) -> Vec<App> {
    (0..count)
        .map(|i| {
            let app = session.app(AppSpec::at(10 * i as i16, 10, 200, 100));
            app.map();
            eventually("focused", || session.focus() == app.window);
            app
        })
        .collect()
}

fn send(session: &Session, command: Command) {
    assert!(session.send(command));
}

const FULL: (i32, i32, u32, u32) = (0, 0, 1280, 800);

/// [Y1] and [Y8]
#[test]
fn y1_one_tiled_window_fills_the_screen() {
    let session = tiling();
    let apps = map(&session, 1);
    eventually("full", || session.outer(apps[0].window) == FULL);
    let g = session.geometry(apps[0].window).unwrap();
    assert_eq!((g.width, g.height, g.border_width), (1274, 794, 3));
}

/// [Y2]
#[test]
fn y2_the_master_is_on_the_right_and_new_windows_join_the_stack() {
    let session = tiling();
    let apps = map(&session, 3);
    eventually("tiled", || {
        session.outer(apps[0].window) == (640, 0, 640, 800)
            && session.outer(apps[1].window) == (0, 0, 640, 400)
            && session.outer(apps[2].window) == (0, 400, 640, 400)
    });
}

/// [Y2] and [Y7]
#[test]
fn y2_the_master_side_and_width_are_set() {
    let master = Master { side: Side::Left, width: 0.6 };
    let session = Session::with_settings(Settings { master, ..Settings::default() });
    let apps = map(&session, 2);
    eventually("tiled", || {
        session.outer(apps[0].window) == (0, 0, 768, 800)
            && session.outer(apps[1].window) == (768, 0, 512, 800)
    });
}

/// [Y3]
#[test]
fn y3_window_height_changes_the_focused_windows_share() {
    let session = tiling();
    let apps = map(&session, 3);
    send(&session, Command::Focus(sugoi::Direction::Previous));
    eventually("b focused", || session.focus() == apps[1].window);
    send(&session, Command::WindowHeight(0.1));
    eventually("b taller", || session.outer(apps[1].window) == (0, 0, 640, 480));
    assert_eq!(session.outer(apps[2].window), (0, 480, 640, 320));
}

/// [Y4]
#[test]
fn y4_master_width_widens_the_master_column() {
    let session = tiling();
    let apps = map(&session, 2);
    send(&session, Command::MasterWidth(0.1));
    eventually("wider", || session.outer(apps[0].window) == (512, 0, 768, 800));
    for _ in 0..20 {
        send(&session, Command::MasterWidth(0.1));
    }
    eventually("at most 0.95", || session.outer(apps[0].window) == (64, 0, 1216, 800));
}

/// [Y5]
#[test]
fn y5_max_fills_the_screen_with_every_window() {
    let session = tiling();
    let apps = map(&session, 2);
    send(&session, Command::Layout(Choice::Pick(Layout::Max)));
    eventually("maxed", || apps.iter().all(|a| session.outer(a.window) == FULL));
    send(&session, Command::Focus(sugoi::Direction::Previous));
    eventually("a raised", || session.above(apps[0].window, apps[1].window));
}

/// [Y6]
#[test]
fn y6_float_leaves_windows_where_the_layout_put_them() {
    let session = tiling();
    let apps = map(&session, 2);
    eventually("tiled", || session.outer(apps[0].window) == (640, 0, 640, 800));
    send(&session, Command::Layout(Choice::Pick(Layout::Float)));
    let c = session.app(AppSpec::at(100, 100, 200, 100));
    c.map();
    eventually("c managed", || session.focus() == c.window);
    assert_eq!(session.outer(apps[0].window), (640, 0, 640, 800));
    assert_eq!(session.outer(apps[1].window), (0, 0, 640, 800));
}

/// [Y7]
#[test]
fn y7_layouts_cycle_from_the_first() {
    let layouts = vec![Layout::Max, Layout::Tile];
    let session = Session::with_settings(Settings { layouts, ..Settings::default() });
    let apps = map(&session, 2);
    eventually("max first", || session.outer(apps[0].window) == FULL);
    send(&session, Command::Layout(Choice::Next));
    eventually("tile next", || session.outer(apps[0].window) == (640, 0, 640, 800));
    send(&session, Command::Layout(Choice::Next));
    eventually("max again", || session.outer(apps[0].window) == FULL);
}

/// [Y9]
#[test]
fn y9_a_tiled_window_keeps_its_cell() {
    let session = tiling();
    let events = EventMask::STRUCTURE_NOTIFY;
    let app = session.app(AppSpec { events, ..AppSpec::at(0, 0, 200, 100) });
    app.map();
    eventually("full", || session.outer(app.window) == FULL);
    while app.conn.poll_for_event().unwrap().is_some() {}
    let request = ConfigureWindowAux::new().x(5).y(5).width(100).height(100);
    app.conn.configure_window(app.window, &request).unwrap();
    app.conn.flush().unwrap();
    let event = app.expect_event("synthetic ConfigureNotify", |e| {
        matches!(e, Event::ConfigureNotify(_)) && e.sent_event()
    });
    let Event::ConfigureNotify(n) = event else { unreachable!() };
    assert_eq!((n.x, n.y, n.width, n.height), (0, 0, 1274, 794));
    assert_eq!(session.outer(app.window), FULL);
}

/// [W1] and [W8]
#[test]
fn w1_floating_leaves_the_layout_and_stacks_above() {
    let session = tiling();
    let apps = map(&session, 2);
    session.move_pointer(1000, 400);
    send(&session, Command::Toggle(State::Floating));
    eventually("a alone in the layout", || session.outer(apps[0].window) == FULL);
    assert_eq!(session.outer(apps[1].window), (0, 0, 640, 800));
    send(&session, Command::Focus(sugoi::Direction::Previous));
    eventually("a focused", || session.focus() == apps[0].window);
    settle();
    assert!(session.above(apps[1].window, apps[0].window));
    send(&session, Command::Focus(sugoi::Direction::Next));
    eventually("b focused", || session.focus() == apps[1].window);
    send(&session, Command::Toggle(State::Floating));
    eventually("tiled again", || session.outer(apps[0].window) == (640, 0, 640, 800));
}

/// [W2]
#[test]
fn w2_a_window_without_a_position_is_centered() {
    let session = Session::with_settings(floating());
    let spec = AppSpec { positioned: false, ..AppSpec::at(0, 0, 200, 100) };
    let app = session.app(spec);
    app.map();
    eventually("centered", || session.outer(app.window) == (537, 347, 206, 106));
}

/// [W2]
#[test]
fn w2_a_floating_window_stays_on_screen() {
    let session = Session::with_settings(floating());
    let app = session.app(AppSpec::at(1200, -50, 200, 100));
    app.map();
    eventually("on screen", || session.outer(app.window) == (1074, 0, 206, 106));
}

/// [W3]
#[test]
fn w3_maximized_fills_the_screen_and_restores() {
    let session = tiling();
    let apps = map(&session, 2);
    send(&session, Command::Toggle(State::Maximized));
    eventually("maximized", || session.outer(apps[1].window) == FULL);
    assert!(session.above(apps[1].window, apps[0].window));
    send(&session, Command::Toggle(State::Maximized));
    eventually("restored", || session.outer(apps[1].window) == (0, 0, 640, 800));
}

/// [W4] and [W5]
#[test]
fn w4_fullscreen_covers_the_screen_without_a_border_above_on_top() {
    let session = tiling();
    let apps = map(&session, 2);
    send(&session, Command::Toggle(State::OnTop));
    send(&session, Command::Focus(sugoi::Direction::Previous));
    eventually("a focused", || session.focus() == apps[0].window);
    send(&session, Command::Toggle(State::Fullscreen));
    eventually("fullscreen", || session.geometry(apps[0].window).unwrap().border_width == 0);
    let g = session.geometry(apps[0].window).unwrap();
    assert_eq!((g.x, g.y, g.width, g.height), (0, 0, 1280, 800));
    assert!(session.above(apps[0].window, apps[1].window));
    send(&session, Command::Toggle(State::Fullscreen));
    eventually("restored", || session.outer(apps[0].window) == (640, 0, 640, 800));
}

/// [W5] and [W8]
#[test]
fn w5_on_top_stays_above_floating_windows() {
    let session = Session::with_settings(floating());
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    let b = session.app(AppSpec::at(100, 100, 300, 200));
    a.map();
    eventually("a", || session.focus() == a.window);
    send(&session, Command::Toggle(State::OnTop));
    // Commands and X events race: let the toggle land before b takes the focus.
    settle();
    b.map();
    eventually("b", || session.focus() == b.window);
    settle();
    assert!(session.above(a.window, b.window));
}

/// [W6]
#[test]
fn w6_minimize_hides_the_window_and_restore_brings_it_back() {
    let session = tiling();
    let apps = map(&session, 2);
    send(&session, Command::Minimize);
    eventually("hidden", || !session.viewable(apps[1].window));
    assert_eq!(session.wm_state(apps[1].window), Some(vec![3, 0]));
    eventually("a alone", || session.outer(apps[0].window) == FULL);
    eventually("a focused", || session.focus() == apps[0].window);
    send(&session, Command::Restore);
    eventually("shown", || session.viewable(apps[1].window));
    eventually("b focused", || session.focus() == apps[1].window);
    assert_eq!(session.wm_state(apps[1].window), Some(vec![1, 0]));
    assert_eq!(session.outer(apps[1].window), (0, 0, 640, 800));
}

/// [W6] and [C2]
#[test]
fn w6_quit_maps_minimized_windows_again() {
    let mut session = tiling();
    let apps = map(&session, 1);
    send(&session, Command::Minimize);
    eventually("hidden", || !session.viewable(apps[0].window));
    send(&session, Command::Quit);
    assert!(session.join().is_ok());
    assert!(session.viewable(apps[0].window));
}

/// [W7]
#[test]
fn w7_center_with_a_size_floats_the_window() {
    let session = tiling();
    let apps = map(&session, 2);
    send(&session, Command::Center(Some(0.7)));
    eventually("centered", || session.outer(apps[1].window) == (192, 120, 896, 560));
    eventually("a alone in the layout", || session.outer(apps[0].window) == FULL);
}

fn mouse() -> Session {
    let buttons = vec![
        Binding::commands("Mod-1", vec![Command::Move]),
        Binding::commands("Mod-3", vec![Command::Resize]),
    ];
    Session::with_settings(Settings { buttons, ..Settings::default() })
}

/// [G1]
#[test]
fn g1_mod_drag_moves_and_floats_the_window() {
    let session = mouse();
    let apps = map(&session, 2);
    eventually("tiled", || session.outer(apps[1].window) == (0, 0, 640, 800));
    session.drag(SUPER, 1, (100, 100), (200, 150));
    eventually("moved", || session.outer(apps[1].window) == (100, 50, 640, 800));
    eventually("a alone in the layout", || session.outer(apps[0].window) == FULL);
}

/// [G2]
#[test]
fn g2_mod_right_drag_resizes_from_the_corner() {
    let session = mouse();
    let apps = map(&session, 2);
    eventually("tiled", || session.outer(apps[1].window) == (0, 0, 640, 800));
    let fake = |kind, detail, x, y| {
        session.conn.xtest_fake_input(kind, detail, 0, session.root, x, y, 0).unwrap();
        session.conn.sync().unwrap();
    };
    fake(MOTION_NOTIFY_EVENT, 0, 100, 100);
    fake(KEY_PRESS_EVENT, SUPER, 0, 0);
    fake(BUTTON_PRESS_EVENT, 3, 0, 0);
    let pointer = || {
        let p = session.conn.query_pointer(session.root).unwrap().reply().unwrap();
        (p.root_x, p.root_y)
    };
    eventually("pointer at the corner", || pointer() == (639, 799));
    fake(MOTION_NOTIFY_EVENT, 0, 689, 749);
    fake(BUTTON_RELEASE_EVENT, 3, 0, 0);
    fake(KEY_RELEASE_EVENT, SUPER, 0, 0);
    eventually("resized", || session.outer(apps[1].window) == (0, 0, 690, 750));
}

/// [Z1]
#[test]
fn z1_arranging_does_not_move_the_focus() {
    let layouts = vec![Layout::Max, Layout::Tile];
    let session = Session::with_settings(Settings { layouts, ..Settings::default() });
    let apps = map(&session, 3);
    session.move_pointer(100, 100);
    settle();
    assert_eq!(session.focus(), apps[2].window);
    // Tiling puts b under the resting pointer.
    send(&session, Command::Layout(Choice::Next));
    eventually("tiled", || session.outer(apps[1].window) == (0, 0, 640, 400));
    settle();
    assert_eq!(session.focus(), apps[2].window);
}

/// [Z2]
#[test]
fn z2_after_a_close_the_window_under_the_pointer_gets_the_focus() {
    let session = tiling();
    session.move_pointer(100, 100);
    let apps = map(&session, 3);
    // The history becomes c, a, b; the pointer rests over b.
    send(&session, Command::Focus(sugoi::Direction::Next));
    eventually("a", || session.focus() == apps[0].window);
    send(&session, Command::Focus(sugoi::Direction::Previous));
    eventually("c", || session.focus() == apps[2].window);
    apps[2].conn.destroy_window(apps[2].window).unwrap();
    apps[2].conn.sync().unwrap();
    eventually("b under the pointer", || session.focus() == apps[1].window);
}

/// [W6] and [M4]
#[test]
fn w6_a_client_withdrawing_a_minimized_window_unmanages_it() {
    let session = tiling();
    let apps = map(&session, 1);
    send(&session, Command::Minimize);
    eventually("iconic", || session.wm_state(apps[0].window) == Some(vec![3, 0]));
    // ICCCM 4.1.4: a client withdraws an unmapped window with a synthetic UnmapNotify.
    let unmap = UnmapNotifyEvent {
        response_type: UNMAP_NOTIFY_EVENT,
        sequence: 0,
        event: session.root,
        window: apps[0].window,
        from_configure: false,
    };
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    apps[0].conn.send_event(false, session.root, mask, unmap).unwrap();
    apps[0].conn.sync().unwrap();
    eventually("withdrawn", || session.wm_state(apps[0].window).is_none());
}

/// [Y3]
#[test]
fn y3_a_new_window_gets_an_average_share() {
    let session = tiling();
    let apps = map(&session, 3);
    send(&session, Command::Focus(sugoi::Direction::Previous));
    eventually("b focused", || session.focus() == apps[1].window);
    send(&session, Command::WindowHeight(0.05));
    eventually("b taller", || session.outer(apps[1].window).3 == 440);
    let d = session.app(AppSpec::at(0, 0, 200, 100));
    d.map();
    // Weights 1.1, 0.9 and 1.0: d gets a third of the column.
    eventually("d a third", || session.outer(d.window) == (0, 533, 640, 267));
}

/// [G1]
#[test]
fn g1_move_drags_the_clicked_window_even_if_it_cannot_take_the_focus() {
    let session = mouse();
    let spec = AppSpec { input: Some(false), ..AppSpec::at(0, 0, 200, 100) };
    let inert = session.app(spec);
    inert.map();
    eventually("inert managed", || session.wm_state(inert.window).is_some());
    let a = session.app(AppSpec::at(0, 0, 200, 100));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    eventually("tiled", || session.outer(inert.window) == (640, 0, 640, 800));
    session.drag(SUPER, 1, (1000, 400), (900, 350));
    eventually("inert moved", || session.outer(inert.window) == (540, -50, 640, 800));
    assert_eq!(session.outer(a.window), FULL);
}

/// [W6]
#[test]
fn w6_a_client_mapping_its_minimized_window_restores_it() {
    let session = tiling();
    let apps = map(&session, 2);
    send(&session, Command::Minimize);
    eventually("iconic", || session.wm_state(apps[1].window) == Some(vec![3, 0]));
    apps[1].map();
    eventually("normal", || session.wm_state(apps[1].window) == Some(vec![1, 0]));
    eventually("focused", || session.focus() == apps[1].window);
    eventually("tiled", || session.outer(apps[1].window) == (0, 0, 640, 800));
}

/// [Y9]
#[test]
fn y9_a_floating_request_for_the_same_geometry_gets_a_reply() {
    let session = Session::with_settings(floating());
    let events = EventMask::STRUCTURE_NOTIFY;
    let app = session.app(AppSpec { events, ..AppSpec::at(100, 100, 200, 100) });
    app.map();
    eventually("placed", || session.outer(app.window) == (100, 100, 206, 106));
    settle();
    while app.conn.poll_for_event().unwrap().is_some() {}
    let request = ConfigureWindowAux::new().x(100).y(100).width(200).height(100);
    app.conn.configure_window(app.window, &request).unwrap();
    app.conn.flush().unwrap();
    app.expect_event("a ConfigureNotify", |e| matches!(e, Event::ConfigureNotify(_)));
}

/// [G1]
#[test]
fn g1_a_move_without_a_press_drags_nothing() {
    let keys = vec![Binding::commands("Mod-x", vec![Command::Move])];
    let session = Session::with_settings(Settings { keys, ..floating() });
    // a takes its clicks, as apps do, so the replayed press stops there.
    let events = EventMask::BUTTON_PRESS;
    let a = session.app(AppSpec { events, ..AppSpec::at(0, 0, 200, 100) });
    let b = session.app(AppSpec::at(400, 0, 200, 100));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    session.click(50, 50);
    eventually("a focused", || session.focus() == a.window);
    session.keys(&[SUPER, common::keycode::X]);
    // Time for a grab, were the move to start one.
    settle();
    session.move_pointer(300, 300);
    settle();
    assert_eq!(session.outer(a.window), (0, 0, 206, 106));
}

/// [Y3]
#[test]
fn y3_after_a_swap_new_windows_still_get_an_average_share() {
    let session = tiling();
    let apps = map(&session, 3);
    send(&session, Command::Focus(sugoi::Direction::Previous));
    eventually("b focused", || session.focus() == apps[1].window);
    for _ in 0..20 {
        send(&session, Command::WindowHeight(0.05));
    }
    eventually("b at most", || session.outer(apps[1].window).3 == 760);
    // b becomes the master; the stack is the old master and c.
    send(&session, Command::Swap(sugoi::Direction::Previous));
    eventually("b master", || session.outer(apps[1].window) == (640, 0, 640, 800));
    let d = session.app(AppSpec::at(0, 0, 200, 100));
    d.map();
    eventually("d a third", || session.outer(d.window).3 >= 160);
    for app in [&apps[0], &apps[2]] {
        assert!(session.outer(app.window).3 >= 40, "{:?}", session.outer(app.window));
    }
}
