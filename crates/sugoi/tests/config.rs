//! Contract cases for `doc/spec/config.md`, one behavior per case. They run the sugoi
//! binary, since Ruby can only start once per process.

mod common;

use std::fs;

use common::keycode::*;
use common::*;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::wrapper::ConnectionExt as _;

/// What the `systemd-run` stub logs for `spawn name argv`.
fn spawned(name: &str, argv: &str) -> String {
    format!("--user --scope --collect --quiet --slice=app-{name}.slice -- {argv}")
}

fn spawns(session: &mut Session) -> Vec<String> {
    session.binary().log("spawn.log")
}

fn notifications(session: &mut Session) -> Vec<String> {
    session.binary().log("notify.log")
}

/// Rewrites the config file.
fn rewrite(session: &mut Session, config: &str) {
    fs::write(&session.binary().config, config).unwrap();
}

/// [L1] and [D1]
#[test]
fn l1_d1_the_config_file_sets_the_border() {
    let session =
        Session::with_config(r##"border width: 7, focused: "#00ff00", normal: "#0000ff""##);
    let a = session.app(AppSpec::at(0, 0, 200, 200));
    let b = session.app(AppSpec::at(300, 0, 200, 200));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    assert_eq!(session.geometry(a.window).unwrap().border_width, 7);
    eventually("colors", || {
        session.border_color(b.window) == 0x00ff00 && session.border_color(a.window) == 0x0000ff
    });
}

/// [L2]
#[test]
fn l2_the_config_is_plain_ruby() {
    let mut session = Session::with_config(
        r#"
        TERMINAL = %w[kitty --single-instance]
        def terminal = spawn(*TERMINAL)
        key("Mod-x") { terminal }
        "#,
    );
    session.keys(&[SUPER, X]);
    eventually("spawned", || spawns(&mut session) == [spawned("kitty", "kitty --single-instance")]);
}

/// [L3]
#[test]
fn l3_without_a_config_file_the_defaults_apply() {
    let mut session = Session::without_config();
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("focused", || session.focus() == app.window);
    assert_eq!(session.geometry(app.window).unwrap().border_width, 3);
    session.keys(&[SUPER, RETURN]);
    eventually("xterm", || spawns(&mut session) == [spawned("xterm", "xterm")]);
    session.keys(&[SUPER, SHIFT, Q]);
    eventually("closed", || !session.exists(app.window));
    assert!(notifications(&mut session).is_empty());
}

/// [L4] and [L3]
#[test]
fn l4_a_config_with_a_syntax_error_falls_back_to_the_defaults() {
    let mut session = Session::with_config("border width: 9\nkey \"Mod-x\" do\n");
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("focused", || session.focus() == app.window);
    assert_eq!(session.geometry(app.window).unwrap().border_width, 3);
    let notes = notifications(&mut session);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("config.rb:"), "{notes:?}");
    session.keys(&[SUPER, SHIFT, Q]);
    eventually("closed by a default binding", || !session.exists(app.window));
}

/// [L4]
#[test]
fn l4_errors_name_the_config_line() {
    let mut session = Session::with_config("border width: 5\nraise \"nope\"\n");
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].ends_with("config.rb:2: nope (RuntimeError)"), "{notes:?}");
}

/// [L4] and [D3]
#[test]
fn l4_an_unknown_key_name_fails_the_load() {
    let mut session = Session::with_config("border width: 5\nkey(\"Mod-Nope\") { close }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("managed", || session.wm_state(app.window).is_some());
    assert_eq!(session.geometry(app.window).unwrap().border_width, 3);
    let notes = notifications(&mut session);
    assert!(notes[0].contains("unknown key name \"Nope\""), "{notes:?}");
}

/// [L4] and [L5]
#[test]
fn l4_a_failed_reload_keeps_the_previous_config() {
    let mut session = Session::with_config(
        "border width: 5\nkey(\"Mod-Ctrl-r\") { reload }\nkey(\"Mod-x\") { spawn \"a\" }\n",
    );
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("border 5", || session.geometry(app.window).unwrap().border_width == 5);
    rewrite(&mut session, "border width: 9\nkey(\"Mod-x\") {\n");
    session.keys(&[SUPER, CTRL, R]);
    eventually("notified", || notifications(&mut session).len() == 1);
    assert_eq!(session.geometry(app.window).unwrap().border_width, 5);
    session.keys(&[SUPER, X]);
    eventually("old binding", || spawns(&mut session) == [spawned("a", "a")]);
}

/// [L5]
#[test]
fn l5_reload_applies_the_new_settings_to_every_window() {
    let mut session = Session::with_config("border width: 5\nkey(\"Mod-Ctrl-r\") { reload }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("border 5", || session.geometry(app.window).unwrap().border_width == 5);
    rewrite(
        &mut session,
        "border width: 9\nkey(\"Mod-Ctrl-r\") { reload }\nkey(\"Mod-x\") { spawn \"b\" }\n",
    );
    session.keys(&[SUPER, CTRL, R]);
    eventually("border 9", || session.geometry(app.window).unwrap().border_width == 9);
    session.keys(&[SUPER, X]);
    eventually("new binding", || spawns(&mut session) == [spawned("b", "b")]);
}

/// [D2]
#[test]
fn d2_modkey_names_what_mod_stands_for() {
    let mut session = Session::with_config("modkey \"Alt\"\nkey(\"Mod-x\") { spawn \"alt\" }\n");
    session.keys(&[SUPER, X]);
    session.keys(&[ALT, X]);
    eventually("spawned", || !spawns(&mut session).is_empty());
    settle();
    assert_eq!(spawns(&mut session), [spawned("alt", "alt")]);
}

/// [D3]
#[test]
fn d3_keycode_chords_bind_physical_keys() {
    let mut session = Session::with_config("key(\"Mod-#10\") { spawn \"one\" }\n");
    session.keys(&[SUPER, ONE]);
    eventually("spawned", || spawns(&mut session) == [spawned("one", "one")]);
}

/// [D3]
#[test]
fn d3_a_later_binding_of_a_chord_replaces_the_earlier() {
    let mut session = Session::with_config(
        "key(\"Mod-x\") { spawn \"first\" }\nkey(\"Mod-x\") { spawn \"second\" }\n",
    );
    session.keys(&[SUPER, X]);
    eventually("spawned", || !spawns(&mut session).is_empty());
    settle();
    assert_eq!(spawns(&mut session), [spawned("second", "second")]);
}

/// [D4]
#[test]
fn d4_a_button_on_a_window_focuses_it_and_consumes_the_press() {
    let session = Session::with_config("layouts :float\nbutton(\"Mod-1\") { close }\n");
    let events = EventMask::BUTTON_PRESS;
    let spec =
        AppSpec { events, protocols: vec!["WM_DELETE_WINDOW"], ..AppSpec::at(0, 0, 200, 200) };
    let a = session.app(spec);
    let b = session.app(AppSpec::at(300, 0, 200, 200));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    session.conn.xtest_fake_input(KEY_PRESS_EVENT, SUPER, 0, session.root, 0, 0, 0).unwrap();
    session.click(50, 50);
    session.conn.xtest_fake_input(KEY_RELEASE_EVENT, SUPER, 0, session.root, 0, 0, 0).unwrap();
    session.conn.sync().unwrap();
    a.expect_protocol("WM_DELETE_WINDOW");
    settle();
    while let Some(event) = a.conn.poll_for_event().unwrap() {
        assert!(!matches!(event, Event::ButtonPress(_)), "the press reached the window");
    }
}

/// [D6]
#[test]
fn d6_a_root_button_runs_its_block() {
    let mut session = Session::with_config("root_button(\"3\") { spawn \"menu\" }\n");
    session.press(900, 500, 3);
    eventually("spawned", || spawns(&mut session) == [spawned("menu", "menu")]);
}

const FOCUS_KEYS: &str = r#"
key("Mod-j") { focus :previous }
key("Mod-k") { focus :next }
key("Mod-Shift-k") { swap :next }
"#;

/// [D5]
#[test]
fn d5_focus_follows_the_window_order_wraps_and_raises() {
    let session = Session::with_config(FOCUS_KEYS);
    let apps: Vec<App> =
        [0, 100, 200].into_iter().map(|x| session.app(AppSpec::at(x, 0, 300, 200))).collect();
    for app in &apps {
        app.map();
        eventually("focused", || session.focus() == app.window);
    }
    session.keys(&[SUPER, K]);
    eventually("wrapped to a", || session.focus() == apps[0].window);
    eventually("a raised", || session.above(apps[0].window, apps[2].window));
    session.keys(&[SUPER, K]);
    eventually("b", || session.focus() == apps[1].window);
    session.keys(&[SUPER, J]);
    eventually("back to a", || session.focus() == apps[0].window);
}

/// [D5]
#[test]
fn d5_swap_changes_the_window_order() {
    let session = Session::with_config(FOCUS_KEYS);
    let apps: Vec<App> =
        [0, 300, 600].into_iter().map(|x| session.app(AppSpec::at(x, 0, 200, 200))).collect();
    for app in &apps {
        app.map();
        eventually("focused", || session.focus() == app.window);
    }
    session.keys(&[SUPER, K]);
    eventually("a", || session.focus() == apps[0].window);
    // The order a, b, c becomes b, a, c; the focus stays on a.
    session.keys(&[SUPER, SHIFT, K]);
    settle();
    assert_eq!(session.focus(), apps[0].window);
    session.keys(&[SUPER, K]);
    eventually("c follows a", || session.focus() == apps[2].window);
}

/// [D5] and [P1]
#[test]
fn d5_p1_spawn_and_sh_run_in_their_own_scope() {
    let mut session = Session::with_config(
        r#"
        key("Mod-x") do
          spawn "kitty"
          spawn "bash", "-ic", "~/bin/superkitty", name: "superkitty"
          sh "overmind restart"
        end
        "#,
    );
    session.keys(&[SUPER, X]);
    eventually("three spawns", || spawns(&mut session).len() == 3);
    // The stubs run concurrently, so their log lines come in any order.
    let mut logged = spawns(&mut session);
    logged.sort();
    let mut expected = [
        spawned("kitty", "kitty"),
        spawned("superkitty", "bash -ic ~/bin/superkitty"),
        spawned("sh", "sh -c overmind restart"),
    ];
    expected.sort();
    assert_eq!(logged, expected);
}

/// [D5]
#[test]
fn d5_quit_exits_with_status_0() {
    let mut session = Session::with_config("key(\"Mod-Shift-e\") { quit }\n");
    session.keys(&[SUPER, SHIFT, E]);
    eventually("exited", || session.binary().status().is_some());
    assert!(session.binary().status().unwrap().success());
}

/// [D5]
#[test]
fn d5_restart_starts_a_fresh_process_that_adopts_the_windows() {
    // A reload runs the file again in the same Ruby, a restart in a new one.
    let mut session = Session::with_config(
        r#"
        $runs = ($runs || 0) + 1
        border width: $runs
        key("Mod-Ctrl-r") { reload }
        key("Mod-x") { restart }
        "#,
    );
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("border 1", || session.geometry(app.window).unwrap().border_width == 1);
    session.keys(&[SUPER, CTRL, R]);
    eventually("border 2", || session.geometry(app.window).unwrap().border_width == 2);
    session.keys(&[SUPER, X]);
    eventually("border 1 again", || session.geometry(app.window).unwrap().border_width == 1);
    assert!(session.wm_running());
    assert_eq!(session.wm_state(app.window), Some(vec![1, 0]));
}

/// [R1]
#[test]
fn r1_a_raising_block_runs_none_of_its_actions() {
    let mut session = Session::with_config(
        "key(\"Mod-x\") do\n  spawn \"a\"\n  raise \"boom\"\nend\nkey(\"Mod-z\") { spawn \"z\" }\n",
    );
    session.keys(&[SUPER, X]);
    eventually("notified", || !notifications(&mut session).is_empty());
    assert!(notifications(&mut session)[0].ends_with("config.rb:3: boom (RuntimeError)"));
    session.keys(&[SUPER, Z]);
    eventually("still running", || spawns(&mut session) == [spawned("z", "z")]);
}

/// [R2]
#[test]
fn r2_a_block_running_over_1_s_is_stopped() {
    let mut session = Session::with_config(
        "key(\"Mod-x\") do\n  spawn \"a\"\n  loop {}\nend\nkey(\"Mod-z\") { spawn \"z\" }\n",
    );
    session.keys(&[SUPER, X]);
    session.keys(&[SUPER, Z]);
    eventually("still running", || spawns(&mut session) == [spawned("z", "z")]);
    let notes = notifications(&mut session);
    assert!(notes[0].contains("execution expired"), "{notes:?}");
}

/// [R3]
#[test]
fn r3_keymap_changes_bind_the_chords_again() {
    let mut session = Session::with_config("key(\"Mod-j\") { spawn \"j\" }\n");
    // Swap the keysyms of the j and z keys, as a layout change would.
    let mapping = session.conn.get_keyboard_mapping(J, Z - J + 1).unwrap().reply().unwrap();
    let per = usize::from(mapping.keysyms_per_keycode);
    let mut keysyms = mapping.keysyms;
    let (j, z) = (0, usize::from(Z - J) * per);
    for i in 0..per {
        keysyms.swap(j + i, z + i);
    }
    session.conn.change_keyboard_mapping(Z - J + 1, J, per as u8, &keysyms).unwrap();
    session.conn.sync().unwrap();
    eventually("j on the z key", || {
        session.keys(&[SUPER, Z]);
        std::thread::sleep(std::time::Duration::from_millis(50));
        !spawns(&mut session).is_empty()
    });
}

/// [R4]
#[test]
fn r4_chords_match_with_num_lock_on() {
    let mut session = Session::with_config("key(\"Mod-x\") { spawn \"x\" }\n");
    session.keys(&[NUM_LOCK]);
    session.keys(&[SUPER, X]);
    eventually("spawned", || spawns(&mut session) == [spawned("x", "x")]);
}

/// [L2] The ported awesome config in `doc/config.rb` loads without an error.
#[test]
fn l2_the_example_config_loads() {
    let example = include_str!("../../../doc/config.rb");
    let mut session = Session::with_config(example);
    session.keys(&[SUPER, RETURN]);
    eventually("kitty", || spawns(&mut session) == [spawned("kitty", "/usr/bin/kitty")]);
    assert!(notifications(&mut session).is_empty());
}

/// [D6]
#[test]
fn d6_root_buttons_leave_the_windows_their_clicks() {
    let mut session = Session::with_config("root_button(\"3\") { spawn \"menu\" }\n");
    let events = EventMask::BUTTON_PRESS;
    let app = session.app(AppSpec { events, ..AppSpec::at(0, 0, 200, 200) });
    app.map();
    eventually("focused", || session.focus() == app.window);
    session.press(50, 50, 3);
    app.expect_event("button 3", |e| matches!(e, Event::ButtonPress(p) if p.detail == 3));
    settle();
    assert!(spawns(&mut session).is_empty());
}

/// [L4]
#[test]
fn l4_a_failed_reload_keeps_a_config_without_keys() {
    let mut session =
        Session::with_config("layouts :float\nborder width: 5\nroot_button(\"3\") { reload }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("border 5", || session.geometry(app.window).unwrap().border_width == 5);
    rewrite(&mut session, "border width: 9\nkey(\"Mod-x\") {\n");
    session.press(900, 500, 3);
    eventually("notified", || notifications(&mut session).len() == 1);
    settle();
    assert_eq!(session.geometry(app.window).unwrap().border_width, 5);
}

/// [L6]
#[test]
fn l6_a_config_still_loading_after_5_s_is_stopped() {
    let mut session = Session::starting("border width: 9\nsleep 30\n");
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    let limit = std::time::Duration::from_secs(10);
    eventually_within(limit, "managed", || session.wm_state(app.window).is_some());
    assert_eq!(session.geometry(app.window).unwrap().border_width, 3);
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].contains("execution expired"), "{notes:?}");
}

/// [L7]
#[test]
fn l7_a_chord_another_program_holds_is_reported() {
    let mut session = Session::with_config("key(\"Mod-Ctrl-r\") { reload }\n");
    let mods = ModMask::M4;
    let grab =
        session.conn.grab_key(false, session.root, mods, X, GrabMode::ASYNC, GrabMode::ASYNC);
    grab.unwrap().check().unwrap();
    rewrite(&mut session, "key(\"Mod-Ctrl-r\") { reload }\nkey(\"Mod-x\") { close }\n");
    session.keys(&[SUPER, CTRL, R]);
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].ends_with("taken by another program: Mod-x"), "{notes:?}");
}

/// [D5] and [F4]
#[test]
fn d5_focus_skips_windows_that_cannot_take_it() {
    let session = Session::with_config(FOCUS_KEYS);
    let a = session.app(AppSpec::at(0, 0, 200, 200));
    let inert = session.app(AppSpec { input: Some(false), ..AppSpec::at(300, 0, 200, 200) });
    let c = session.app(AppSpec::at(600, 0, 200, 200));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    inert.map();
    eventually("inert managed", || session.wm_state(inert.window).is_some());
    c.map();
    eventually("c focused", || session.focus() == c.window);
    session.keys(&[SUPER, K]);
    eventually("wrapped to a", || session.focus() == a.window);
    session.keys(&[SUPER, K]);
    eventually("past inert to c", || session.focus() == c.window);
}

/// [P1]
#[test]
fn p1_spawned_apps_are_not_sugois_children() {
    let mut session = Session::with_config("key(\"Mod-x\") { spawn \"sleep\", \"10\" }\n");
    session.keys(&[SUPER, X]);
    eventually("spawned", || spawns(&mut session) == [spawned("sleep", "sleep 10")]);
    let pid = session.binary().pid().to_string();
    let children = || {
        let out = std::process::Command::new("pgrep").args(["-P", &pid]).output().unwrap();
        String::from_utf8(out.stdout).unwrap()
    };
    eventually("no children", || children().trim().is_empty());
}

/// [C3]
#[test]
fn c3_sigterm_quits_like_quit() {
    let mut session = Session::with_config("");
    let app = session.app(AppSpec::at(0, 0, 200, 200));
    app.map();
    eventually("border 3", || session.geometry(app.window).unwrap().border_width == 3);
    let pid = session.binary().pid().to_string();
    std::process::Command::new("kill").args(["-TERM", &pid]).status().unwrap();
    eventually("exited", || session.binary().status().is_some());
    assert!(session.binary().status().unwrap().success());
    assert_eq!(session.geometry(app.window).unwrap().border_width, 0);
}

/// [C3]
#[test]
fn c3_a_config_cannot_trap_the_quit_signals() {
    let mut session = Session::with_config("trap(\"TERM\") {}\n");
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].ends_with("config.rb:1: sugoi handles SIGTERM (ArgumentError)"), "{notes:?}");
}

/// [Y7] and [Y2]
#[test]
fn y7_the_config_sets_the_layouts_and_the_master() {
    let session = Session::with_config(
        "layouts :max, :tile\nmaster side: :left, width: 0.6\nkey(\"Mod-x\") { layout :next }\n",
    );
    let a = session.app(AppSpec::at(0, 0, 200, 100));
    let b = session.app(AppSpec::at(0, 0, 200, 100));
    a.map();
    b.map();
    eventually("b focused", || session.focus() == b.window);
    eventually("max first", || session.outer(a.window) == (0, 0, 1280, 800));
    session.keys(&[SUPER, X]);
    eventually("tile, master left", || session.outer(a.window) == (0, 0, 768, 800));
}

/// [W7] and [D5]
#[test]
fn w7_the_layout_actions_queue_commands() {
    let session = Session::with_config("key(\"Mod-x\") { center size: 0.5 }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 100));
    app.map();
    eventually("full", || session.outer(app.window) == (0, 0, 1280, 800));
    session.keys(&[SUPER, X]);
    eventually("centered", || session.outer(app.window) == (320, 200, 640, 400));
}

/// [Y7] and [L4]
#[test]
fn y7_an_unknown_layout_fails_the_load() {
    let mut session = Session::with_config("layouts :tile, :spiral\n");
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].contains("expected one of :tile, :max, :float, not :spiral"), "{notes:?}");
}

/// [Y7] and [L4]
#[test]
fn y7_a_master_width_out_of_range_fails_the_load() {
    let mut session = Session::with_config("master width: 1.2\n");
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].contains("master width must be between 0.05 and 0.95"), "{notes:?}");
}

/// [V4] and [V8]
#[test]
fn v4_the_workspace_actions_queue_commands() {
    let session =
        Session::with_config("key(\"Mod-x\") { workspace 2 }\nkey(\"Mod-z\") { move_to 12 }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 100));
    app.map();
    eventually("focused", || session.focus() == app.window);
    session.keys(&[SUPER, Z]);
    eventually("moved", || !session.viewable(app.window));
    session.keys(&[SUPER, X]);
    session.keys(&[SUPER, X]);
    eventually("on 12, shown", || session.viewable(app.window));
}

/// [V4] and [R1]
#[test]
fn v4_a_workspace_out_of_range_is_an_error() {
    let mut session = Session::with_config("key(\"Mod-x\") { workspace 31 }\n");
    session.keys(&[SUPER, X]);
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].contains("workspaces are numbered 1 to 30, not 31"), "{notes:?}");
}

/// [U2] [U3] The rule DSL, with the awesome config's second-browser hook ([H1], [H3]).
#[test]
fn h1_the_second_browser_on_a_workspace_floats() {
    let session = Session::with_config(
        r#"
        rule class: "Thunar", float: true
        BROWSERS = %w[Chromium firefox]
        on(:manage) do |window|
          next unless BROWSERS.include?(window.class_name)
          if windows(workspace: window.workspace).count { |w| BROWSERS.include?(w.class_name) } > 1
            window.float
            window.size 600, 400
            window.center
          end
        end
        "#,
    );
    let first = session.app(AppSpec::at(0, 0, 200, 100).class("Navigator", "firefox"));
    first.map();
    eventually("first tiled", || session.outer(first.window) == (0, 0, 1280, 800));
    let second = session.app(AppSpec::at(0, 0, 200, 100).class("chromium", "Chromium"));
    second.map();
    eventually("second floats", || session.outer(second.window) == (337, 197, 606, 406));
    assert_eq!(session.outer(first.window), (0, 0, 1280, 800));
    let thunar = session.app(AppSpec::at(10, 10, 200, 100).class("thunar", "Thunar"));
    thunar.map();
    eventually("thunar floats", || session.focus() == thunar.window);
    assert_eq!(session.outer(thunar.window), (10, 10, 206, 106));
}

/// [H1] [H2]
#[test]
fn h1_hooks_see_read_only_windows_and_workspaces() {
    let mut session = Session::with_config(
        r#"
        key("Mod-x") { workspace 2 }
        on(:focus) { |w| spawn "focus", w.class_name, w.instance, w.title, w.type.to_s, w.transient?.to_s, w.workspace.to_s, w.floating?.to_s, w.frozen?.to_s }
        on(:unmanage) { |w| spawn "gone", w.class_name }
        on(:workspace) { |n| spawn "ws", n.to_s }
        "#,
    );
    // The harness's marker window ran the focus and unmanage hooks already.
    eventually("marker seen", || spawns(&mut session).len() == 2);
    let spec = AppSpec { title: Some("hello"), ..AppSpec::at(0, 0, 200, 100) };
    let app = session.app(spec.class("kitty", "kitty"));
    app.map();
    eventually("focus hook", || spawns(&mut session).len() == 3);
    assert_eq!(
        spawns(&mut session)[2],
        spawned("focus", "focus kitty kitty hello normal false 1 false true")
    );
    session.keys(&[SUPER, X]);
    eventually("workspace hook", || spawns(&mut session).len() == 4);
    app.conn.destroy_window(app.window).unwrap();
    app.conn.flush().unwrap();
    eventually("unmanage hook", || spawns(&mut session).len() == 5);
    let mut logged = spawns(&mut session)[3..].to_vec();
    logged.sort();
    assert_eq!(logged, [spawned("gone", "gone kitty"), spawned("ws", "ws 2")]);
}

/// [H4]
#[test]
fn h4_hooks_do_not_run_for_their_own_actions() {
    let mut session = Session::with_config(
        "key(\"Mod-x\") { focus :next }\non(:focus) { |w| spawn \"seen\"; focus :next }\n",
    );
    // The harness's marker ran the hook once.
    eventually("marker seen", || spawns(&mut session).len() == 1);
    let a = session.app(AppSpec::at(0, 0, 200, 100));
    a.map();
    eventually("seen for a", || spawns(&mut session).len() == 2);
    let b = session.app(AppSpec::at(0, 0, 200, 100));
    b.map();
    // b's focus runs the hook, whose `focus :next` moves the focus to a without a hook.
    eventually("seen for b", || spawns(&mut session).len() == 3);
    settle();
    assert_eq!(spawns(&mut session).len(), 3);
    assert_eq!(session.focus(), a.window);
}

/// [H4] and [R1]
#[test]
fn h4_a_raising_hook_is_reported() {
    let mut session = Session::with_config("on(:manage) { |w| raise \"no #{w.class_name}\" }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 100).class("kitty", "kitty"));
    app.map();
    let ending = "config.rb:1: no kitty (RuntimeError)";
    eventually("notified", || notifications(&mut session).iter().any(|n| n.ends_with(ending)));
    eventually("still managed and shown", || session.viewable(app.window));
}

/// [U2]
#[test]
fn u2_an_unknown_rule_option_fails_the_load() {
    let mut session = Session::with_config("rule klass: \"x\", float: true\n");
    eventually("notified", || !notifications(&mut session).is_empty());
    let notes = notifications(&mut session);
    assert!(notes[0].contains("unknown rule option :klass"), "{notes:?}");
}

/// [H1] The workspace hook runs after the switch, focus included.
#[test]
fn h1_the_workspace_hook_sees_the_new_workspaces_focus() {
    let session = Session::with_config(
        "key(\"Mod-x\") { workspace 2 }\nkey(\"Mod-z\") { workspace 1 }\n\
         on(:workspace) { |n| close if n == 1 }\n",
    );
    let a = session.app(AppSpec::at(0, 0, 200, 100));
    a.map();
    eventually("a focused", || session.focus() == a.window);
    session.keys(&[SUPER, X]);
    let b = session.app(AppSpec::at(0, 0, 200, 100));
    b.map();
    eventually("b focused", || session.focus() == b.window);
    session.keys(&[SUPER, Z]);
    eventually("a closed", || !session.exists(a.window));
    assert!(session.exists(b.window));
}

/// [H4] A hook's reload stops the event's other hooks, whose blocks it replaced.
#[test]
fn h4_a_reload_in_a_hook_runs_no_stale_block() {
    let mut session =
        Session::with_config("on(:focus) { reload }\non(:focus) { spawn \"second\" }\n");
    eventually("marker done", || {
        session.focus() == POINTER_ROOT || !spawns(&mut session).is_empty()
    });
    settle();
    let before = spawns(&mut session).len();
    rewrite(&mut session, "key(\"Mod-y\") { spawn \"y\" }\nkey(\"Mod-x\") { spawn \"stale\" }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 100));
    app.map();
    eventually("focused", || session.focus() == app.window);
    settle();
    assert_eq!(spawns(&mut session).len(), before, "{:?}", spawns(&mut session));
}

/// [H2] and [W7]
#[test]
fn h2_window_center_floats_a_tiled_window() {
    let session = Session::with_config("on(:manage) { |w| w.size 300, 200; w.center }\n");
    let app = session.app(AppSpec::at(0, 0, 200, 100));
    app.map();
    eventually("floating, centered", || session.outer(app.window) == (487, 297, 306, 206));
}

/// [U6] A restart keeps a ruled window where the user put it.
#[test]
fn u6_an_adopted_window_keeps_its_geometry() {
    let session = Session::with_config(
        "rule class: \"feh\", float: true, size: [300, 200], center: true\n\
         key(\"Mod-x\") { restart }\n",
    );
    let feh = session.app(AppSpec::at(0, 0, 100, 100).class("feh", "feh"));
    feh.map();
    eventually("centered", || session.outer(feh.window) == (487, 297, 306, 206));
    let request = ConfigureWindowAux::new().x(10).y(20);
    feh.conn.configure_window(feh.window, &request).unwrap();
    feh.conn.flush().unwrap();
    eventually("moved", || session.outer(feh.window) == (10, 20, 306, 206));
    session.keys(&[SUPER, X]);
    // The restart gives the window its own border back for a moment, then sugoi's.
    std::thread::sleep(std::time::Duration::from_millis(500));
    eventually("kept", || session.outer(feh.window) == (10, 20, 306, 206));
}
