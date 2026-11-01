//! Contract cases for `doc/spec/apps.md`, one behavior per case. Kills run against
//! real systemd scopes the cases start (`sugoi-test-*.scope`, an xterm), never one a
//! test process is in: test windows of the test process itself never get a `kill`.
//! Without a user manager or xterm they pass vacuously.

mod common;

use common::*;
use sugoi::{Command, Settings};

/// A session where the scope's xterm has the focus.
fn with_xterm(slice: &str) -> Option<(Session, Scope)> {
    let session = Session::with_settings(Settings::default());
    let scope = Scope::start(slice, &session.x.display)?;
    eventually("xterm focused", || session.focus() > POINTER_ROOT);
    Some((session, scope))
}

/// [A2] [A3] [A4]
#[test]
fn a4_kill_kills_the_apps_scope() {
    let Some((session, mut scope)) = with_xterm("app-sugoitest.slice") else { return };
    assert!(session.send(Command::Kill));
    eventually("the app was killed", || scope.killed());
}

/// [A4] A scope outside the user manager's app.slice is not the app's own: close.
#[test]
fn a4_kill_closes_a_window_outside_app_slice() {
    let Some((session, mut scope)) = with_xterm("session.slice") else { return };
    assert!(session.send(Command::Kill));
    // xterm takes WM_DELETE_WINDOW and exits on its own, not by SIGKILL.
    eventually("closed", || !scope.running());
    assert!(!scope.killed());
}
