//! Contract cases for `doc/spec/workspaces.md`, one behavior per case. Workspaces are
//! numbered 1 to 30 in commands and 0 to 29 in EWMH properties.

mod common;

use common::*;
use sugoi::{Choice, Command, Direction, Effects, Goto, Layout, Matcher, Rule, Settings, State};
use x11rb::protocol::xproto::*;

fn session() -> Session {
    Session::with_settings(Settings::default())
}

fn app(session: &Session) -> App {
    let app = session.app(AppSpec::at(0, 0, 200, 100));
    app.map();
    eventually("focused", || session.focus() == app.window);
    app
}

fn send(session: &Session, command: Command) {
    assert!(session.send(command));
}

fn go(session: &Session, goto: Goto) {
    send(session, Command::Workspace(goto));
}

fn cardinal(session: &Session, window: Window, name: &str) -> Option<u32> {
    let atom = session.atom(name);
    let reply = session.conn.get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1);
    reply.ok()?.reply().ok()?.value32()?.next()
}

fn current(session: &Session) -> Option<u32> {
    cardinal(session, session.root, "_NET_CURRENT_DESKTOP")
}

fn desktop(session: &Session, window: Window) -> Option<u32> {
    cardinal(session, window, "_NET_WM_DESKTOP")
}

/// [V1] and [V9]
#[test]
fn v1_new_windows_go_on_the_current_workspace() {
    let session = session();
    let a = app(&session);
    assert_eq!(desktop(&session, a.window), Some(0));
    go(&session, Goto::Number(4));
    eventually("on 4", || current(&session) == Some(3));
    let b = app(&session);
    assert_eq!(desktop(&session, b.window), Some(3));
}

/// [V2]
#[test]
fn v2_only_the_current_workspaces_windows_show() {
    let session = session();
    let a = app(&session);
    go(&session, Goto::Number(2));
    eventually("a hidden", || !session.viewable(a.window));
    assert_eq!(session.wm_state(a.window), Some(vec![3, 0]));
    let b = app(&session);
    eventually("b alone", || session.outer(b.window) == (0, 0, 1280, 800));
    go(&session, Goto::Number(1));
    eventually("a shown", || session.viewable(a.window));
    assert_eq!(session.wm_state(a.window), Some(vec![1, 0]));
    eventually("b hidden", || !session.viewable(b.window));
    assert_eq!(session.outer(a.window), (0, 0, 1280, 800));
}

/// [V2] and [D5]
#[test]
fn v2_focus_and_restore_stay_on_the_current_workspace() {
    let session = session();
    let a = app(&session);
    send(&session, Command::Minimize);
    eventually("a minimized", || !session.viewable(a.window));
    go(&session, Goto::Number(2));
    eventually("on 2", || current(&session) == Some(1));
    let b = app(&session);
    send(&session, Command::Restore);
    send(&session, Command::Focus(Direction::Next));
    settle();
    assert!(!session.viewable(a.window));
    assert_eq!(session.focus(), b.window);
}

/// [V3]
#[test]
fn v3_each_workspace_has_its_own_layout() {
    let session = session();
    let apps = [app(&session), app(&session)];
    send(&session, Command::Layout(sugoi::Choice::Pick(Layout::Max)));
    eventually("max", || session.outer(apps[0].window) == (0, 0, 1280, 800));
    go(&session, Goto::Number(2));
    let c = [app(&session), app(&session)];
    eventually("tile on 2", || session.outer(c[0].window) == (640, 0, 640, 800));
    go(&session, Goto::Back);
    eventually("max on 1", || session.viewable(apps[0].window));
    assert_eq!(session.outer(apps[0].window), (0, 0, 1280, 800));
}

/// [V4]
#[test]
fn v4_repeating_a_number_cycles_through_its_bank() {
    let session = session();
    for expected in [2, 12, 22, 2] {
        go(&session, Goto::Number(3));
        eventually("cycled", || current(&session) == Some(expected));
    }
    go(&session, Goto::Number(5));
    eventually("5", || current(&session) == Some(4));
    go(&session, Goto::Number(17));
    eventually("17 directly", || current(&session) == Some(16));
    go(&session, Goto::Number(7));
    eventually("7 from 17", || current(&session) == Some(26));
}

/// [V5]
#[test]
fn v5_next_and_previous_wrap() {
    let session = session();
    go(&session, Goto::Previous);
    eventually("30", || current(&session) == Some(29));
    go(&session, Goto::Next);
    eventually("1", || current(&session) == Some(0));
}

/// [V6]
#[test]
fn v6_back_returns_to_the_previous_workspace() {
    let session = session();
    go(&session, Goto::Number(4));
    eventually("4", || current(&session) == Some(3));
    go(&session, Goto::Number(9));
    eventually("9", || current(&session) == Some(8));
    go(&session, Goto::Back);
    eventually("back to 4", || current(&session) == Some(3));
    go(&session, Goto::Back);
    eventually("back to 9", || current(&session) == Some(8));
}

/// [V7]
#[test]
fn v7_a_switch_focuses_the_workspaces_last_focused_window() {
    let session = session();
    let a = app(&session);
    let _b = app(&session);
    send(&session, Command::Focus(Direction::Previous));
    eventually("a", || session.focus() == a.window);
    go(&session, Goto::Number(2));
    eventually("root", || session.focus() == POINTER_ROOT);
    go(&session, Goto::Back);
    eventually("a again", || session.focus() == a.window);
    // The pointer rests over the other window: the switch's crossings move no focus.
    settle();
    assert_eq!(session.focus(), a.window);
}

/// [V8]
#[test]
fn v8_move_to_hides_the_window_and_stays() {
    let session = session();
    let a = app(&session);
    let b = app(&session);
    send(&session, Command::MoveTo(3));
    eventually("b hidden", || !session.viewable(b.window));
    assert_eq!(desktop(&session, b.window), Some(2));
    // Still on workspace 1, which sugoi has not switched away from: no property yet.
    assert_eq!(current(&session).unwrap_or(0), 0);
    eventually("a focused", || session.focus() == a.window);
    eventually("a alone", || session.outer(a.window) == (0, 0, 1280, 800));
    go(&session, Goto::Number(3));
    eventually("b shown", || session.viewable(b.window));
}

/// [V8] and [V4]
#[test]
fn v8_move_to_cycles_from_the_windows_workspace() {
    let session = session();
    go(&session, Goto::Number(3));
    eventually("3", || current(&session) == Some(2));
    let a = app(&session);
    send(&session, Command::MoveTo(3));
    eventually("a on 13", || desktop(&session, a.window) == Some(12));
    assert_eq!(current(&session), Some(2));
}

/// [V9] and [V11]
#[test]
fn v9_workspaces_survive_a_restart() {
    let mut session = session();
    let a = app(&session);
    go(&session, Goto::Number(2));
    let b = app(&session);
    send(&session, Command::Minimize);
    eventually("b minimized", || !session.viewable(b.window));
    send(&session, Command::Quit);
    assert!(session.join().is_ok());
    assert!(session.viewable(a.window) && session.viewable(b.window));
    session.start_wm_with(Settings::default());
    eventually("on 2", || current(&session) == Some(1));
    eventually("a hidden on 1", || !session.viewable(a.window));
    assert_eq!(desktop(&session, a.window), Some(0));
    // b stays minimized ([V11]), on 2.
    assert!(!session.viewable(b.window));
    send(&session, Command::Restore);
    eventually("b shows on 2", || session.viewable(b.window));
}

/// Sets the `_NET_WM_DESKTOP` `app` asks for, then maps its window.
fn map_on(app: &App, desktop: u32) {
    use x11rb::wrapper::ConnectionExt as _;
    let atom = app.atom("_NET_WM_DESKTOP");
    app.conn
        .change_property32(PropMode::REPLACE, app.window, atom, AtomEnum::CARDINAL, &[desktop])
        .unwrap();
    app.map();
}

/// [V10]
#[test]
fn v10_a_new_window_goes_on_the_workspace_its_app_set() {
    let session = session();
    let a = app(&session);
    let b = session.app(AppSpec::at(0, 0, 200, 100));
    map_on(&b, 4);
    eventually("b managed", || session.wm_state(b.window).is_some());
    assert_eq!(desktop(&session, b.window), Some(4));
    assert_eq!(session.wm_state(b.window), Some(vec![3, 0]));
    assert!(!session.viewable(b.window));
    assert_eq!(session.focus(), a.window);
    assert_eq!(current(&session), Some(0));
    go(&session, Goto::Number(5));
    eventually("b shows on 5", || session.viewable(b.window));
}

/// [V10]
#[test]
fn v10_an_invalid_workspace_is_the_current_one() {
    let session = session();
    go(&session, Goto::Number(2));
    eventually("on 2", || current(&session) == Some(1));
    for asked in [30, 0xFFFF_FFFF] {
        let a = session.app(AppSpec::at(0, 0, 200, 100));
        map_on(&a, asked);
        eventually("focused", || session.focus() == a.window);
        assert_eq!(desktop(&session, a.window), Some(1));
    }
}

/// [V2] and [M4], the property's shrunk case: an app withdraws its window while sugoi
/// still has a switch to handle, so sugoi's own unmap finds it unmapped already.
#[test]
fn v2_a_withdrawal_racing_a_switch_unmanages_the_window() {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xtest::ConnectionExt as _;
    let session = session();
    let a = app(&session);
    let conn = &session.conn;
    // Under a grab the key is queued for sugoi first, the unmap second.
    conn.grab_server().unwrap();
    for (kind, key) in [
        (KEY_PRESS_EVENT, common::keycode::SUPER),
        (KEY_PRESS_EVENT, common::keycode::ONE + 1),
        (KEY_RELEASE_EVENT, common::keycode::ONE + 1),
        (KEY_RELEASE_EVENT, common::keycode::SUPER),
    ] {
        conn.xtest_fake_input(kind, key, 0, session.root, 0, 0, 0).unwrap();
    }
    conn.unmap_window(a.window).unwrap();
    conn.ungrab_server().unwrap();
    conn.flush().unwrap();
    eventually("on 2", || current(&session) == Some(1));
    eventually("withdrawn", || session.wm_state(a.window).is_none());
    assert_eq!(desktop(&session, a.window), None);
}

/// [V9]
#[test]
fn v9_the_root_names_the_current_workspace_from_the_start() {
    let session = session();
    assert_eq!(current(&session), Some(0));
}

/// Quits sugoi and starts it again with `settings`, as a restart does.
fn restart(session: &mut Session, settings: Settings) {
    send(session, Command::Quit);
    assert!(session.join().is_ok());
    session.start_wm_with(settings);
}

fn outers(session: &Session, apps: &[&App]) -> Vec<(i32, i32, u32, u32)> {
    apps.iter().map(|a| session.outer(a.window)).collect()
}

/// [V11]
#[test]
fn v11_a_restart_keeps_the_window_order() {
    let mut session = session();
    let (a, b, c) = (app(&session), app(&session), app(&session));
    send(&session, Command::Swap(Direction::Previous));
    let all = [&a, &b, &c];
    eventually("c above b", || session.outer(c.window) == (0, 0, 640, 400));
    let before = outers(&session, &all);
    restart(&mut session, Settings::default());
    eventually("the same places", || outers(&session, &all) == before);
}

/// [V11]
#[test]
fn v11_floating_windows_keep_their_state_geometry_and_stacking() {
    let mut session = session();
    let tiled = app(&session);
    let a = app(&session);
    send(&session, Command::Center(Some(0.5)));
    let b = app(&session);
    send(&session, Command::Center(Some(0.4)));
    eventually("b floats", || session.outer(b.window) == (384, 240, 512, 320));
    eventually("tiled alone", || session.outer(tiled.window) == (0, 0, 1280, 800));
    assert!(session.above(b.window, a.window));
    let all = [&tiled, &a, &b];
    let before = outers(&session, &all);
    restart(&mut session, Settings::default());
    eventually("the same places", || outers(&session, &all) == before);
    assert!(session.above(b.window, a.window));
}

/// [V11]
#[test]
fn v11_minimized_windows_stay_minimized() {
    let mut session = session();
    let a = app(&session);
    let b = app(&session);
    send(&session, Command::Minimize);
    eventually("b minimized", || !session.viewable(b.window));
    restart(&mut session, Settings::default());
    eventually("a full", || session.outer(a.window) == (0, 0, 1280, 800));
    assert!(!session.viewable(b.window));
    assert_eq!(session.wm_state(b.window), Some(vec![3, 0]));
    send(&session, Command::Restore);
    eventually("b back", || session.viewable(b.window));
}

/// [V11]
#[test]
fn v11_layouts_widths_shares_and_back_survive() {
    let layouts = || Settings { layouts: vec![Layout::Tile, Layout::Max], ..Settings::default() };
    let mut session = Session::with_settings(layouts());
    let (a, b, c) = (app(&session), app(&session), app(&session));
    send(&session, Command::MasterWidth(0.1));
    send(&session, Command::WindowHeight(0.1));
    go(&session, Goto::Number(2));
    eventually("on 2", || current(&session) == Some(1));
    let (d, e) = (app(&session), app(&session));
    send(&session, Command::Layout(Choice::Next));
    eventually("max", || session.outer(d.window) == (0, 0, 1280, 800));
    go(&session, Goto::Number(1));
    eventually("on 1", || current(&session) == Some(0));
    let all = [&a, &b, &c];
    eventually("a wider", || session.outer(a.window).2 == 768);
    let before = outers(&session, &all);
    restart(&mut session, layouts());
    eventually("the same places", || outers(&session, &all) == before);
    go(&session, Goto::Back);
    eventually("back on 2", || current(&session) == Some(1));
    eventually("still max", || session.outer(e.window) == (0, 0, 1280, 800));
    assert_eq!(session.outer(d.window), (0, 0, 1280, 800));
}

/// [V11] and [U6]
#[test]
fn v11_the_kept_state_wins_over_rules() {
    let effects = Effects { float: Some(true), ..Effects::default() };
    let class = Matcher { class: vec!["chat".into()], ..Matcher::default() };
    let settings = || Settings {
        rules: vec![Rule { matcher: class.clone(), effects: effects.clone() }],
        ..Settings::default()
    };
    let mut session = Session::with_settings(settings());
    let a = app(&session);
    let chat = session.app(AppSpec::at(0, 0, 200, 100).class("chat", "chat"));
    chat.map();
    eventually("chat focused", || session.focus() == chat.window);
    send(&session, Command::Toggle(State::Floating));
    eventually("chat tiled", || session.outer(chat.window) == (0, 0, 640, 800));
    restart(&mut session, settings());
    eventually("a master", || session.outer(a.window) == (640, 0, 640, 800));
    eventually("chat still tiled", || session.outer(chat.window) == (0, 0, 640, 800));
}

/// [V11] and [S2]
#[test]
fn v11_windows_the_state_does_not_name_come_after() {
    let mut session = session();
    let (a, b) = (app(&session), app(&session));
    send(&session, Command::Quit);
    assert!(session.join().is_ok());
    // A window mapped while no WM runs: on top, so [S2] alone would put it first.
    let late = session.app(AppSpec::at(0, 0, 200, 100));
    late.map();
    session.start_wm_with(Settings::default());
    eventually("a still the master", || session.outer(a.window) == (640, 0, 640, 800));
    eventually("late last", || session.outer(late.window) == (0, 400, 640, 400));
    assert_eq!(session.outer(b.window), (0, 0, 640, 400));
}

/// [V11] and [Y7]
#[test]
fn v11_a_kept_layout_the_cycle_lacks_gives_way() {
    let settings = Settings { layouts: vec![Layout::Max, Layout::Tile], ..Settings::default() };
    let mut session = Session::with_settings(settings);
    let (a, b) = (app(&session), app(&session));
    eventually("max", || session.outer(a.window) == (0, 0, 1280, 800));
    restart(&mut session, Settings { layouts: vec![Layout::Tile], ..Settings::default() });
    eventually("tile", || session.outer(b.window) == (0, 0, 640, 800));
}

/// [V11]
#[test]
fn v11_a_change_just_before_a_quit_is_kept() {
    let mut session = session();
    let a = app(&session);
    let b = app(&session);
    // Both at once: sugoi handles the quit before it would next wait.
    send(&session, Command::Minimize);
    send(&session, Command::Quit);
    assert!(session.join().is_ok());
    session.start_wm_with(Settings::default());
    eventually("a full", || session.outer(a.window) == (0, 0, 1280, 800));
    assert!(!session.viewable(b.window));
}

/// [V11] and [W3]
#[test]
fn v11_a_maximized_window_restores_its_kept_geometry() {
    let mut session = session();
    let _tiled = app(&session);
    let a = app(&session);
    send(&session, Command::Center(Some(0.5)));
    eventually("a floats", || session.outer(a.window) == (320, 200, 640, 400));
    send(&session, Command::Toggle(State::Maximized));
    eventually("maximized", || session.outer(a.window) == (0, 0, 1280, 800));
    restart(&mut session, Settings::default());
    eventually("adopted", || session.wm_state(a.window).is_some());
    session.click(640, 400);
    eventually("a focused", || session.focus() == a.window);
    send(&session, Command::Toggle(State::Maximized));
    eventually("back where it floated", || session.outer(a.window) == (320, 200, 640, 400));
}

/// [V11] and [V7]
#[test]
fn v11_the_focus_history_survives() {
    let floating = || Settings { layouts: vec![Layout::Float], ..Settings::default() };
    let mut session = Session::with_settings(floating());
    let a = session.app(AppSpec::at(0, 0, 300, 200));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    let b = session.app(AppSpec::at(600, 0, 300, 200));
    b.map();
    eventually("b focused", || session.focus() == b.window);
    // The pointer focuses a without raising it: b stays on top, a is focused last.
    session.move_pointer(100, 100);
    eventually("a focused again", || session.focus() == a.window);
    session.move_pointer(1000, 700);
    assert!(session.above(b.window, a.window));
    restart(&mut session, floating());
    eventually("adopted", || session.wm_state(b.window).is_some());
    go(&session, Goto::Number(2));
    eventually("on 2", || current(&session) == Some(1));
    go(&session, Goto::Number(1));
    eventually("a, focused last", || session.focus() == a.window);
}

/// Sets the root's kept state, as a previous sugoi would have.
fn kept_state(session: &Session, text: &str) {
    use x11rb::wrapper::ConnectionExt as _;
    let (atom, utf8) = (session.atom("_SUGOI_STATE"), session.atom("UTF8_STRING"));
    session
        .conn
        .change_property8(PropMode::REPLACE, session.root, atom, utf8, text.as_bytes())
        .unwrap();
    session.conn.sync().unwrap();
}

/// [V11]
#[test]
fn v11_a_window_reusing_an_id_does_not_inherit_its_state() {
    let mut session = Session::without_wm();
    let a = session.app(AppSpec::at(0, 0, 200, 100));
    a.map();
    // Saved for another process: floating and minimized there.
    let w = a.window;
    kept_state(
        &session,
        &format!(
            "sugoi-state 1\norder {w:#x}\nminimized {w:#x}\nwindow {w:#x} 1 10 10 300 200 1 1 1\n"
        ),
    );
    session.start_wm_with(Settings::default());
    eventually("a tiled, shown", || session.outer(a.window) == (0, 0, 1280, 800));
    assert!(session.viewable(a.window));
}

/// [V11]
#[test]
fn v11_a_name_repeated_in_the_state_counts_once() {
    let mut session = Session::without_wm();
    let a = session.app(AppSpec::at(0, 0, 200, 100));
    let b = session.app(AppSpec::at(0, 0, 200, 100));
    a.map();
    b.map();
    let (wa, wb) = (a.window, b.window);
    // Window lines without a process (an older state) name their windows.
    let lines = format!("window {wa:#x} 0 0 0 9 9 1\nwindow {wb:#x} 0 0 0 9 9 1\n");
    let order = format!("order {wa:#x} {wa:#x} {wb:#x}\nminimized {wb:#x} {wb:#x}\n");
    kept_state(&session, &format!("sugoi-state 1\n{order}{lines}"));
    session.start_wm_with(Settings::default());
    eventually("a alone in the layout", || session.outer(a.window) == (0, 0, 1280, 800));
    assert!(!session.viewable(b.window));
    send(&session, Command::Restore);
    eventually("b back", || session.viewable(b.window));
    eventually("a master", || session.outer(a.window) == (640, 0, 640, 800));
    assert_eq!(session.outer(b.window), (0, 0, 640, 800));
    // No stale entry: the next start shows b as it is now.
    restart(&mut session, Settings::default());
    eventually("b still shown", || session.outer(b.window) == (0, 0, 640, 800));
    assert!(session.viewable(b.window));
}
