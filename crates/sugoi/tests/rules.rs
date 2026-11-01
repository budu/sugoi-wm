//! Contract cases for the rules of `doc/spec/rules.md`, one behavior per case. Hooks
//! need Ruby: their cases are in `config.rs`.

mod common;

use common::*;
use sugoi::{Effects, Matcher, Rule, Settings, WindowType};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;

const FULL: (i32, i32, u32, u32) = (0, 0, 1280, 800);

fn ruled(rules: Vec<Rule>) -> Session {
    Session::with_settings(Settings { rules, ..Settings::default() })
}

fn class(name: &str) -> Matcher {
    Matcher { class: vec![name.into()], ..Matcher::default() }
}

fn float() -> Effects {
    Effects { float: Some(true), ..Effects::default() }
}

fn mapped(session: &Session, spec: AppSpec) -> App {
    let app = session.app(spec);
    app.map();
    eventually("managed", || session.wm_state(app.window).is_some());
    app
}

fn desktop(session: &Session, window: Window) -> Option<u32> {
    let atom = session.atom("_NET_WM_DESKTOP");
    let reply = session.conn.get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1);
    reply.ok()?.reply().ok()?.value32()?.next()
}

/// [U1] [U2] [U3]
#[test]
fn u2_a_rule_by_class_floats_the_window() {
    let session = ruled(vec![Rule { matcher: class("Thunar"), effects: float() }]);
    let terminal = mapped(&session, AppSpec::at(0, 0, 200, 100).class("kitty", "kitty"));
    let thunar = mapped(&session, AppSpec::at(100, 100, 200, 100).class("thunar", "Thunar"));
    eventually("terminal alone in the layout", || session.outer(terminal.window) == FULL);
    assert_eq!(session.outer(thunar.window), (100, 100, 206, 106));
}

/// [U1] [U2]
#[test]
fn u2_every_given_matcher_must_match() {
    let matcher = Matcher {
        class: vec!["firefox".into(), "Chromium".into()],
        instance: vec!["Navigator".into()],
        title: vec!["Picture-in-Picture".into()],
        ..Matcher::default()
    };
    let session = ruled(vec![Rule { matcher, effects: float() }]);
    let spec = AppSpec { title: Some("Picture-in-Picture"), ..AppSpec::at(100, 100, 200, 100) };
    let pip = mapped(&session, spec.clone().class("Navigator", "firefox"));
    let other =
        mapped(&session, AppSpec { title: Some("Mozilla"), ..spec }.class("Navigator", "firefox"));
    eventually("other alone in the layout", || session.outer(other.window) == FULL);
    assert_eq!(session.outer(pip.window), (100, 100, 206, 106));
}

/// [U3]
#[test]
fn u3_size_and_center_place_the_floating_window() {
    let effects = Effects { size: Some((400, 300)), center: true, ..float() };
    let session = ruled(vec![Rule { matcher: class("feh"), effects }]);
    let feh = mapped(&session, AppSpec::at(0, 0, 200, 100).class("feh", "feh"));
    eventually("centered", || session.outer(feh.window) == (437, 247, 406, 306));
}

/// [U3]
#[test]
fn u3_master_puts_the_window_first() {
    let effects = Effects { master: true, ..Effects::default() };
    let session = ruled(vec![Rule { matcher: class("Emacs"), effects }]);
    let a = mapped(&session, AppSpec::at(0, 0, 200, 100));
    let emacs = mapped(&session, AppSpec::at(0, 0, 200, 100).class("emacs", "Emacs"));
    eventually("emacs is the master", || session.outer(emacs.window) == (640, 0, 640, 800));
    assert_eq!(session.outer(a.window), (0, 0, 640, 800));
}

/// [U3] and [U7]
#[test]
fn u7_a_window_sent_elsewhere_stays_hidden_and_unfocused() {
    let effects = Effects { workspace: Some(3), ..Effects::default() };
    let session = ruled(vec![Rule { matcher: class("Caprine"), effects }]);
    let a = mapped(&session, AppSpec::at(0, 0, 200, 100));
    eventually("a focused", || session.focus() == a.window);
    let chat = mapped(&session, AppSpec::at(0, 0, 200, 100).class("caprine", "Caprine"));
    assert_eq!(desktop(&session, chat.window), Some(2));
    settle();
    assert!(!session.viewable(chat.window));
    assert_eq!(session.focus(), a.window);
}

/// [U3] and [W5]
#[test]
fn u3_on_top_keeps_the_window_above() {
    let effects = Effects { on_top: Some(true), ..float() };
    let session = ruled(vec![Rule { matcher: class("pip"), effects }]);
    let pip = mapped(&session, AppSpec::at(0, 0, 200, 100).class("pip", "pip"));
    let b = mapped(&session, AppSpec::at(0, 0, 200, 100).class("b", "b"));
    eventually("b focused", || session.focus() == b.window);
    settle();
    assert!(session.above(pip.window, b.window));
}

/// [U4]
#[test]
fn u4_a_later_rule_replaces_an_earlier_effect() {
    let tile = Effects { float: Some(false), ..Effects::default() };
    let session = ruled(vec![
        Rule { matcher: class("Gimp"), effects: float() },
        Rule { matcher: Matcher { title: vec!["main".into()], ..class("Gimp") }, effects: tile },
    ]);
    let spec = AppSpec { title: Some("main"), ..AppSpec::at(100, 100, 200, 100) };
    let main = mapped(&session, spec.class("gimp", "Gimp"));
    eventually("tiled", || session.outer(main.window) == FULL);
}

/// [U5]
#[test]
fn u5_dialogs_and_transients_float_by_default() {
    let session = ruled(Vec::new());
    let main = mapped(&session, AppSpec::at(0, 0, 200, 100));
    let kind = Some("_NET_WM_WINDOW_TYPE_DIALOG");
    let dialog = mapped(&session, AppSpec { kind, ..AppSpec::at(100, 100, 200, 100) });
    let spec = AppSpec { transient_for: Some(main.window), ..AppSpec::at(300, 100, 200, 100) };
    let transient = mapped(&session, spec);
    eventually("main alone in the layout", || session.outer(main.window) == FULL);
    assert_eq!(session.outer(dialog.window), (100, 100, 206, 106));
    assert_eq!(session.outer(transient.window), (300, 100, 206, 106));
}

/// [U5]
#[test]
fn u5_a_config_rule_can_tile_dialogs() {
    let matcher = Matcher { types: vec![WindowType::Dialog], ..Matcher::default() };
    let effects = Effects { float: Some(false), ..Effects::default() };
    let session = ruled(vec![Rule { matcher, effects }]);
    let kind = Some("_NET_WM_WINDOW_TYPE_DIALOG");
    let dialog = mapped(&session, AppSpec { kind, ..AppSpec::at(100, 100, 200, 100) });
    eventually("tiled", || session.outer(dialog.window) == FULL);
}

/// [U6], as Zed may do: the class comes after the map.
#[test]
fn u6_rules_apply_again_when_the_class_comes_late() {
    let effects = Effects { master: true, ..Effects::default() };
    let session = ruled(vec![Rule { matcher: class("dev.zed.Zed"), effects }]);
    let a = mapped(&session, AppSpec::at(0, 0, 200, 100));
    let zed = mapped(&session, AppSpec::at(0, 0, 200, 100));
    eventually("zed in the stack", || session.outer(zed.window) == (0, 0, 640, 800));
    set_class(&zed.conn, zed.window, "dev.zed.Zed", "dev.zed.Zed");
    zed.conn.flush().unwrap();
    eventually("zed is the master", || session.outer(zed.window) == (640, 0, 640, 800));
    assert_eq!(session.outer(a.window), (0, 0, 640, 800));
}

/// [U6] and [V9]
#[test]
fn u6_an_adopted_window_keeps_its_workspace_over_a_rule() {
    let effects = Effects { workspace: Some(3), ..Effects::default() };
    let rules = vec![Rule { matcher: class("chat"), effects }];
    let mut session = Session::without_wm();
    let chat = session.app(AppSpec::at(0, 0, 200, 100).class("chat", "chat"));
    chat.map();
    let atom = session.atom("_NET_WM_DESKTOP");
    session
        .conn
        .change_property32(PropMode::REPLACE, chat.window, atom, AtomEnum::CARDINAL, &[0])
        .unwrap();
    session.conn.sync().unwrap();
    session.start_wm_with(Settings { rules, ..Settings::default() });
    eventually("managed", || session.wm_state(chat.window).is_some());
    assert_eq!(desktop(&session, chat.window), Some(0));
    assert!(session.viewable(chat.window));
}
