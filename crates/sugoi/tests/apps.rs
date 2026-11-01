//! Contract cases for `doc/spec/apps.md`, one behavior per case. Kills run against
//! real systemd scopes the cases start (`sugoi-test-*.scope`, `sleep 30`), never one a
//! test process itself is in; without a user manager they pass vacuously.

mod common;

use common::*;
use sugoi::{Command, Settings};

fn session() -> Session {
    Session::with_settings(Settings::default())
}

/// A window whose process is `pid` on `machine`, focused, with WM_DELETE_WINDOW.
fn window_of(session: &Session, pid: u32, machine: String) -> App {
    let spec = AppSpec {
        process: Some((pid, machine)),
        protocols: vec!["WM_DELETE_WINDOW"],
        ..AppSpec::at(0, 0, 200, 100)
    };
    let app = session.app(spec);
    app.map();
    eventually("focused", || session.focus() == app.window);
    app
}

/// [A3] [A4]
#[test]
fn a4_kill_stops_the_apps_scope() {
    let Some(mut scope) = Scope::start("app-sugoitest.slice") else { return };
    let session = session();
    let _app = window_of(&session, scope.pid(), host());
    assert!(session.send(Command::Kill));
    eventually("the app ended", || !scope.running());
}

/// [A4] A scope outside the user manager's app.slice is not the app's own.
#[test]
fn a4_kill_closes_a_window_outside_app_slice() {
    let Some(mut scope) = Scope::start("session.slice") else { return };
    let session = session();
    let app = window_of(&session, scope.pid(), host());
    assert!(session.send(Command::Kill));
    app.expect_protocol("WM_DELETE_WINDOW");
    assert!(scope.running());
}

/// [A2] A process ID from another machine means nothing here.
#[test]
fn a2_a_process_on_another_machine_is_unknown() {
    let Some(mut scope) = Scope::start("app-sugoitest.slice") else { return };
    let session = session();
    let app = window_of(&session, scope.pid(), "elsewhere".into());
    assert!(session.send(Command::Kill));
    app.expect_protocol("WM_DELETE_WINDOW");
    assert!(scope.running());
}

/// [A2] [A4] A window without a process closes.
#[test]
fn a4_kill_closes_a_window_without_a_process() {
    let session = session();
    let spec = AppSpec { protocols: vec!["WM_DELETE_WINDOW"], ..AppSpec::at(0, 0, 200, 100) };
    let app = session.app(spec);
    app.map();
    eventually("focused", || session.focus() == app.window);
    assert!(session.send(Command::Kill));
    app.expect_protocol("WM_DELETE_WINDOW");
}
