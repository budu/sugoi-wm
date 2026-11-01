//! Contract cases for `doc/spec/workspaces.md`, one behavior per case. Workspaces are
//! numbered 1 to 30 in commands and 0 to 29 in EWMH properties.

mod common;

use common::*;
use sugoi::{Command, Direction, Goto, Layout, Settings};
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

/// [V9]
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
    assert!(session.viewable(b.window));
    assert_eq!(desktop(&session, a.window), Some(0));
}
