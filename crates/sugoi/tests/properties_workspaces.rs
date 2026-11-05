//! A history property for workspaces, minimized windows, focus and tiling, against
//! sugoi in-process on its own Xvfb.
//!
//! Generated: sequences of up to 24 operations from a small menu (map a plain window, one
//! a rule sends to workspace 2 or one a rule floats ([U2], [U3], [U7]), each maybe
//! asking for workspace 1, 2, 3 or an invalid one, by property or message ([V10]), close
//! one, `workspace` 1 to 3, which cycle on to 11 to 23, `move_to` 1 to 3, back, next,
//! previous, minimize, restore, focus next, next layout, a click on a shown window, a
//! swap, centering the focused window (it floats), and a restart: a quit, then a
//! start, after which every shown window is where it was, [V11]), on at most 6 windows, so
//! cycling, emptied workspaces and minimized windows on hidden workspaces collide often.
//!
//! Checked after every operation, against a reference model written from the specs:
//! the current workspace ([V4] to [V6]); for each window, its workspace ([V8] to [V10]),
//! whether it shows and its `WM_STATE` ([V2], [W6]); the focus: exactly the window the
//! spec names after a map ([F1]), focus next ([D5]), a switch ([V7]) or a restore
//! ([W6]), and some shown window, or the root without one, after a close, minimize or
//! move, where the pointer may decide ([Z2]); tiled cells covering the screen without
//! overlap, or every window full-screen in max ([Y2], [Y5]); in X's own stacking, every
//! shown floating window above every shown tiled one ([W8]).
//!
//! Every operation goes through X (keys by XTEST, apps mapping and destroying), so the
//! server orders them; a barrier then waits until sugoi has handled them all.

mod common;

use common::keycode::*;
use common::*;
use proptest::prelude::*;
use sugoi::{Binding, Choice, Command, Direction, Effects, Goto, Matcher, Rule, Settings};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::wrapper::ConnectionExt as _;

const MAX_WINDOWS: usize = 6;
const SCREEN: (u32, u32) = (1280, 800);
const ESCAPE: u8 = 9;
const LEFT: u8 = 113;
const RIGHT: u8 = 114;
const N: u8 = 57;
const SPACE: u8 = 65;
const C: u8 = 54;

/// What a mapped window is, for the rules in `settings` ([U2], [U3], [U7]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Plain,
    /// A rule sends it to workspace 2.
    Elsewhere,
    /// A rule floats it.
    Floater,
}

/// [V10] The workspace index an app asks for before mapping its window, and how.
#[derive(Clone, Copy, Debug)]
enum Asked {
    Nothing,
    Property(u32),
    Message(u32),
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Map(Class, Asked),
    Close(usize),
    /// The app unmaps one of its shown windows (ICCCM withdrawal).
    Withdraw(usize),
    /// The same, while sugoi has a switch to n still to handle: under a server grab the
    /// key comes first, the unmap second.
    WithdrawSwitching(usize, u32),
    Workspace(u32),
    MoveTo(u32),
    Back,
    Next,
    Previous,
    Minimize,
    Restore,
    FocusNext,
    LayoutNext,
    /// [F3] A click in the middle of a shown window: it raises what is there.
    Click(usize),
    /// [D5] The focused window swaps with the next.
    Swap,
    /// [W7] The focused window floats, centered.
    Center,
    /// [V11] Quit, then start again.
    Restart,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (
            prop::sample::select(vec![Class::Plain, Class::Plain, Class::Elsewhere, Class::Floater]),
            asked(),
        )
            .prop_map(|(class, asked)| Op::Map(class, asked)),
        2 => (0usize..MAX_WINDOWS).prop_map(Op::Close),
        1 => (0usize..MAX_WINDOWS).prop_map(Op::Withdraw),
        1 => (0usize..MAX_WINDOWS, 1u32..=3).prop_map(|(i, n)| Op::WithdrawSwitching(i, n)),
        3 => (1u32..=3).prop_map(Op::Workspace),
        2 => (1u32..=3).prop_map(Op::MoveTo),
        1 => Just(Op::Back),
        1 => Just(Op::Next),
        1 => Just(Op::Previous),
        2 => Just(Op::Minimize),
        2 => Just(Op::Restore),
        2 => Just(Op::FocusNext),
        1 => Just(Op::LayoutNext),
        3 => (0usize..MAX_WINDOWS).prop_map(Op::Click),
        1 => Just(Op::Swap),
        2 => Just(Op::Center),
        1 => Just(Op::Restart),
    ]
}

/// Mostly nothing; otherwise workspace 1 to 3, or 30 (invalid).
fn asked() -> impl Strategy<Value = Asked> {
    let index = prop::sample::select(vec![0u32, 1, 2, 30]);
    prop_oneof![
        2 => Just(Asked::Nothing),
        1 => index.clone().prop_map(Asked::Property),
        1 => index.prop_map(Asked::Message),
    ]
}

/// The keys for every operation that is a command.
fn settings() -> Settings {
    let extra = [
        ("Mod-Escape", Command::Workspace(Goto::Back)),
        ("Mod-Right", Command::Workspace(Goto::Next)),
        ("Mod-Left", Command::Workspace(Goto::Previous)),
        ("Mod-n", Command::Minimize),
        ("Mod-Ctrl-n", Command::Restore),
        ("Mod-space", Command::Layout(Choice::Next)),
        ("Mod-Shift-k", Command::Swap(Direction::Next)),
        ("Mod-c", Command::Center(None)),
    ];
    let mut settings = Settings::default();
    for (chord, command) in extra {
        settings.keys.push(Binding::commands(chord, vec![command]));
    }
    let class = |name: &str| Matcher { class: vec![name.into()], ..Matcher::default() };
    settings.rules = vec![
        Rule {
            matcher: class("elsewhere"),
            effects: Effects { workspace: Some(2), ..Effects::default() },
        },
        Rule {
            matcher: class("floater"),
            effects: Effects { float: Some(true), ..Effects::default() },
        },
    ];
    settings
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Tile,
    Max,
    Float,
}

/// A window as the model sees it.
struct Model {
    window: Window,
    workspace: usize,
    minimized: bool,
    floating: bool,
}

/// The reference model: windows in managed order, workspaces as indices 0 to 29.
struct Reference {
    windows: Vec<Model>,
    current: usize,
    back: usize,
    /// Most recently focused first, as observed.
    history: Vec<Window>,
    /// Minimized windows, latest last.
    minimized: Vec<Window>,
    /// Windows their apps withdrew: sugoi must leave nothing of its own on them.
    withdrawn: Vec<Window>,
    layouts: [Kind; 30],
}

/// [V4], from the spec's words: n's class is n, n+10, n+20; from inside it the next one
/// in that order, wrapping, that holds a modeled window (minimized ones too) or is n,
/// else the current one; from outside, n; 11 to 30 are exact. Indices 0 to 29.
fn spec_target(current: usize, n: u32, model: &Reference) -> usize {
    let n = n as usize;
    if n > 10 {
        return n - 1;
    }
    let used = |w: usize| model.windows.iter().any(|m| m.workspace + 1 == w);
    let class = [n, n + 10, n + 20];
    match class.iter().position(|&w| w == current + 1) {
        Some(i) => {
            (1..=2).map(|k| class[(i + k) % 3]).find(|&w| w == n || used(w)).unwrap_or(current + 1)
                - 1
        }
        None => n - 1,
    }
}

impl Reference {
    fn shown(&self, w: &Model) -> bool {
        w.workspace == self.current && !w.minimized
    }

    fn get(&mut self, window: Window) -> &mut Model {
        self.windows.iter_mut().find(|w| w.window == window).expect("modeled")
    }

    /// Returns whether it is a switch at all ([V6]).
    fn switch(&mut self, target: usize) -> bool {
        if target == self.current {
            return false;
        }
        self.back = self.current;
        self.current = target;
        true
    }

    /// [V7] The focus a switch gives.
    fn switch_focus(&self) -> Option<Window> {
        self.history
            .iter()
            .copied()
            .find(|&h| self.windows.iter().any(|w| w.window == h && self.shown(w)))
    }
}

/// What sugoi shows.
struct Observed {
    current: usize,
    focus: Window,
}

struct Run {
    session: Session,
    apps: Vec<App>,
    barrier: Window,
    width: u16,
}

impl Run {
    fn new() -> Run {
        let session = Session::with_settings(settings());
        // An unmapped window of the test's own: its configure requests go to sugoi,
        // which applies them, unmanaged, after everything sent before.
        let barrier = session.conn.generate_id().unwrap();
        let aux = CreateWindowAux::new();
        session
            .conn
            .create_window(
                0,
                barrier,
                session.root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &aux,
            )
            .unwrap();
        session.conn.sync().unwrap();
        Run { session, apps: Vec::new(), barrier, width: 1 }
    }

    fn barrier(&mut self) {
        self.width = self.width % 1000 + 1;
        let aux = ConfigureWindowAux::new().width(u32::from(self.width));
        self.session.conn.configure_window(self.barrier, &aux).unwrap();
        self.session.conn.sync().unwrap();
        let (session, barrier, width) = (&self.session, self.barrier, self.width);
        eventually("barrier", || session.geometry(barrier).unwrap().width == width);
    }

    fn keys(&self, keys: &[u8]) {
        let conn = &self.session.conn;
        for &key in keys {
            conn.xtest_fake_input(KEY_PRESS_EVENT, key, 0, self.session.root, 0, 0, 0).unwrap();
        }
        for &key in keys.iter().rev() {
            conn.xtest_fake_input(KEY_RELEASE_EVENT, key, 0, self.session.root, 0, 0, 0).unwrap();
        }
        conn.sync().unwrap();
    }

    fn observe(&self) -> Observed {
        let atom = self.session.atom("_NET_CURRENT_DESKTOP");
        let reply = self.session.conn.get_property(
            false,
            self.session.root,
            atom,
            AtomEnum::CARDINAL,
            0,
            1,
        );
        let current = reply.unwrap().reply().unwrap().value32().and_then(|mut v| v.next());
        Observed { current: current.unwrap_or(0) as usize, focus: self.session.focus() }
    }

    fn desktop(&self, window: Window) -> Option<u32> {
        let atom = self.session.atom("_NET_WM_DESKTOP");
        let reply = self.session.conn.get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1);
        reply.unwrap().reply().unwrap().value32().and_then(|mut v| v.next())
    }
}

/// Runs `ops`, checking the model after each; an error names the step.
fn history(ops: &[Op]) -> Result<(), TestCaseError> {
    let mut run = Run::new();
    let mut model = Reference {
        windows: Vec::new(),
        current: 0,
        back: 0,
        history: Vec::new(),
        minimized: Vec::new(),
        withdrawn: Vec::new(),
        layouts: [Kind::Tile; 30],
    };
    // Withdrawn windows' apps stay connected, so their windows still exist.
    let mut kept: Vec<App> = Vec::new();
    for (step, &op) in ops.iter().enumerate() {
        // `None`: any shown window may have the focus; `Some(w)`: exactly w.
        let mut expected_focus: Option<Option<Window>> = None;
        let focused = run.observe().focus;
        match op {
            Op::Map(class, asked) => {
                if model.windows.len() == MAX_WINDOWS {
                    continue;
                }
                let spec = AppSpec::at(0, 0, 100, 100);
                let app = run.session.app(match class {
                    Class::Plain => spec,
                    Class::Elsewhere => spec.class("elsewhere", "elsewhere"),
                    Class::Floater => spec.class("floater", "floater"),
                });
                let atom = app.atom("_NET_WM_DESKTOP");
                let index = match asked {
                    Asked::Nothing => None,
                    Asked::Property(index) => {
                        app.conn
                            .change_property32(
                                PropMode::REPLACE,
                                app.window,
                                atom,
                                AtomEnum::CARDINAL,
                                &[index],
                            )
                            .unwrap();
                        Some(index as usize)
                    }
                    Asked::Message(index) => {
                        let event =
                            ClientMessageEvent::new(32, app.window, atom, [index, 1, 0, 0, 0]);
                        let mask =
                            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
                        app.conn.send_event(false, run.session.root, mask, event).unwrap();
                        Some(index as usize)
                    }
                };
                app.map();
                let window = app.window;
                run.apps.push(app);
                // [U6] A rule's workspace, then the app's if valid, then the current.
                let workspace = match index {
                    _ if class == Class::Elsewhere => 1,
                    Some(index) if index < 30 => index,
                    _ => model.current,
                };
                let floating = class == Class::Floater;
                model.windows.push(Model { window, workspace, minimized: false, floating });
                // [F1], or [U7]: a window sent elsewhere leaves the focus where it was.
                expected_focus = Some(if workspace == model.current {
                    Some(window)
                } else {
                    model
                        .windows
                        .iter()
                        .any(|w| w.window == focused && model.shown(w))
                        .then_some(focused)
                });
            }
            Op::Close(i) => {
                if run.apps.is_empty() {
                    continue;
                }
                let app = run.apps.remove(i % run.apps.len());
                app.conn.destroy_window(app.window).unwrap();
                app.conn.sync().unwrap();
                model.windows.retain(|w| w.window != app.window);
                model.history.retain(|&w| w != app.window);
                model.minimized.retain(|&w| w != app.window);
            }
            Op::Withdraw(i) | Op::WithdrawSwitching(i, _) => {
                let shown: Vec<usize> = (0..run.apps.len())
                    .filter(|&a| {
                        model
                            .windows
                            .iter()
                            .any(|w| w.window == run.apps[a].window && model.shown(w))
                    })
                    .collect();
                if shown.is_empty() {
                    continue;
                }
                let app = run.apps.remove(shown[i % shown.len()]);
                let conn = &run.session.conn;
                if let Op::WithdrawSwitching(_, n) = op {
                    conn.grab_server().unwrap();
                    for (kind, key) in [(KEY_PRESS_EVENT, SUPER), (KEY_PRESS_EVENT, 9 + n as u8)]
                        .into_iter()
                        .chain([(KEY_RELEASE_EVENT, 9 + n as u8), (KEY_RELEASE_EVENT, SUPER)])
                    {
                        conn.xtest_fake_input(kind, key, 0, run.session.root, 0, 0, 0).unwrap();
                    }
                }
                conn.unmap_window(app.window).unwrap();
                if let Op::WithdrawSwitching(..) = op {
                    conn.ungrab_server().unwrap();
                }
                conn.sync().unwrap();
                // sugoi takes the key first, while the window still counts ([V4]).
                let goes = match op {
                    Op::WithdrawSwitching(_, n) => Some(spec_target(model.current, n, &model)),
                    _ => None,
                };
                model.windows.retain(|w| w.window != app.window);
                model.history.retain(|&w| w != app.window);
                model.withdrawn.push(app.window);
                if let Some(goes) = goes
                    && model.switch(goes)
                {
                    expected_focus = Some(model.switch_focus());
                }
                kept.push(app);
            }
            Op::Workspace(n) => {
                run.keys(&[SUPER, 9 + n as u8]);
                if model.switch(spec_target(model.current, n, &model)) {
                    expected_focus = Some(model.switch_focus());
                }
            }
            Op::MoveTo(n) => {
                run.keys(&[SUPER, SHIFT, 9 + n as u8]);
                if let Some(w) = model.windows.iter().position(|w| w.window == focused) {
                    let from = model.windows[w].workspace;
                    model.windows[w].workspace = spec_target(from, n, &model);
                }
            }
            Op::Back => {
                run.keys(&[SUPER, ESCAPE]);
                let back = model.back;
                if model.switch(back) {
                    expected_focus = Some(model.switch_focus());
                }
            }
            Op::Next | Op::Previous => {
                let (key, target) = match op {
                    Op::Next => (RIGHT, (model.current + 1) % 30),
                    _ => (LEFT, (model.current + 29) % 30),
                };
                run.keys(&[SUPER, key]);
                model.switch(target);
                expected_focus = Some(model.switch_focus());
                // Next and previous always switch: 30 workspaces.
            }
            Op::Minimize => {
                run.keys(&[SUPER, N]);
                if model.windows.iter().any(|w| w.window == focused) {
                    model.get(focused).minimized = true;
                    model.minimized.push(focused);
                }
            }
            Op::Restore => {
                run.keys(&[SUPER, CTRL, N]);
                let current = model.current;
                let last = model.minimized.iter().rev().copied().find(|&m| {
                    model.windows.iter().any(|w| w.window == m && w.workspace == current)
                });
                if let Some(window) = last {
                    model.minimized.retain(|&m| m != window);
                    model.get(window).minimized = false;
                    expected_focus = Some(Some(window));
                }
            }
            Op::FocusNext => {
                run.keys(&[SUPER, K]);
                let shown: Vec<Window> =
                    model.windows.iter().filter(|w| model.shown(w)).map(|w| w.window).collect();
                if !shown.is_empty() {
                    let next = match shown.iter().position(|&w| w == focused) {
                        Some(i) => shown[(i + 1) % shown.len()],
                        None => shown[0],
                    };
                    expected_focus = Some(Some(next));
                }
            }
            Op::Restart => {
                let shown: Vec<Window> =
                    model.windows.iter().filter(|w| model.shown(w)).map(|w| w.window).collect();
                let places =
                    |run: &Run| -> Vec<_> { shown.iter().map(|&w| run.session.outer(w)).collect() };
                let before = places(&run);
                assert!(run.session.send(Command::Quit));
                run.session.join().map_err(|e| TestCaseError::fail(e.to_string()))?;
                run.session.start_wm_with(settings());
                run.barrier();
                // [V11] Nothing moved: the same windows in the same places.
                let after = places(&run);
                prop_assert_eq!(&before, &after, "a restart moved windows, step {}", step);
                // [S2] The pointer's window, or the history's: some shown window.
            }
            Op::Click(i) => {
                let shown: Vec<Window> =
                    model.windows.iter().filter(|w| model.shown(w)).map(|w| w.window).collect();
                if shown.is_empty() {
                    continue;
                }
                let (x, y, w, h) = run.session.outer(shown[i % shown.len()]);
                let (cx, cy) = ((x + w as i32 / 2) as i16, (y + h as i32 / 2) as i16);
                run.session.click(cx.clamp(0, 1279), cy.clamp(0, 799));
                // Whatever is on top there takes the focus: some shown window.
            }
            Op::Swap => {
                run.keys(&[SUPER, SHIFT, K]);
            }
            Op::Center => {
                run.keys(&[SUPER, C]);
                if let Some(w) = model.windows.iter_mut().find(|w| w.window == focused) {
                    w.floating = true;
                }
            }
            Op::LayoutNext => {
                run.keys(&[SUPER, SPACE]);
                let layout = &mut model.layouts[model.current];
                *layout = match layout {
                    Kind::Tile => Kind::Max,
                    Kind::Max => Kind::Float,
                    Kind::Float => Kind::Tile,
                };
            }
        }
        run.barrier();
        check(&run, &mut model, expected_focus).map_err(|e| {
            TestCaseError::fail(format!("after step {step} ({op:?}) of {ops:?}: {e}"))
        })?;
    }
    Ok(())
}

fn check(
    run: &Run,
    model: &mut Reference,
    expected_focus: Option<Option<Window>>,
) -> Result<(), String> {
    let observed = run.observe();
    if observed.current != model.current {
        return Err(format!(
            "on workspace {}, expected {}",
            observed.current + 1,
            model.current + 1
        ));
    }
    let shown: Vec<Window> =
        model.windows.iter().filter(|w| model.shown(w)).map(|w| w.window).collect();
    for w in &model.windows {
        let visible = model.shown(w);
        if run.session.viewable(w.window) != visible {
            return Err(format!("{:#x} viewable is not {visible}", w.window));
        }
        let state = if visible { 1 } else { 3 };
        if run.session.wm_state(w.window) != Some(vec![state, 0]) {
            return Err(format!(
                "{:#x} WM_STATE {:?}, expected {state}",
                w.window,
                run.session.wm_state(w.window)
            ));
        }
        if run.desktop(w.window) != Some(w.workspace as u32) {
            return Err(format!(
                "{:#x} on {:?}, expected {}",
                w.window,
                run.desktop(w.window),
                w.workspace
            ));
        }
    }
    for &window in &model.withdrawn {
        if run.session.wm_state(window).is_some() || run.desktop(window).is_some() {
            return Err(format!("withdrawn {window:#x} kept WM_STATE or _NET_WM_DESKTOP"));
        }
    }
    // The focus.
    let focus = observed.focus;
    match expected_focus {
        Some(Some(window)) if focus != window => {
            return Err(format!("focus {focus:#x}, expected {window:#x}"));
        }
        Some(None) | None if shown.is_empty() && focus != POINTER_ROOT => {
            return Err(format!("focus {focus:#x}, expected the root"));
        }
        None if !shown.is_empty() && !shown.contains(&focus) => {
            return Err(format!("focus {focus:#x} is not a shown window of {shown:x?}"));
        }
        _ => {}
    }
    if shown.contains(&focus) {
        model.history.retain(|&w| w != focus);
        model.history.insert(0, focus);
    }
    // [W8] In X's own order, every shown floating window is above every shown tiled one.
    let children = run.session.conn.query_tree(run.session.root).unwrap().reply().unwrap();
    let position = |w: Window| children.children.iter().position(|&c| c == w);
    let lowest_floating = model
        .windows
        .iter()
        .filter(|w| model.shown(w) && w.floating)
        .filter_map(|w| position(w.window).map(|p| (p, w.window)))
        .min();
    let highest_tiled = model
        .windows
        .iter()
        .filter(|w| model.shown(w) && !w.floating)
        .filter_map(|w| position(w.window).map(|p| (p, w.window)))
        .max();
    if let (Some((f, floating)), Some((t, tiled))) = (lowest_floating, highest_tiled)
        && f < t
    {
        return Err(format!("floating {floating:#x} is under tiled {tiled:#x} in X"));
    }
    // The geometry of the shown windows the layout places ([U3]: floaters float).
    let tiled: Vec<Window> =
        model.windows.iter().filter(|w| model.shown(w) && !w.floating).map(|w| w.window).collect();
    let rects: Vec<(i32, i32, u32, u32)> = tiled.iter().map(|&w| run.session.outer(w)).collect();
    match model.layouts[model.current] {
        Kind::Tile if !rects.is_empty() => {
            let area: u64 = rects.iter().map(|r| u64::from(r.2) * u64::from(r.3)).sum();
            if area != u64::from(SCREEN.0) * u64::from(SCREEN.1) {
                return Err(format!("tiles cover {area} px: {rects:?}"));
            }
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    let overlap = a.0 < b.0 + b.2 as i32
                        && b.0 < a.0 + a.2 as i32
                        && a.1 < b.1 + b.3 as i32
                        && b.1 < a.1 + a.3 as i32;
                    if overlap {
                        return Err(format!("tiles overlap: {rects:?}"));
                    }
                }
            }
        }
        Kind::Max => {
            if let Some(r) = rects.iter().find(|r| **r != (0, 0, SCREEN.0, SCREEN.1)) {
                return Err(format!("max window at {r:?}"));
            }
        }
        _ => {}
    }
    Ok(())
}

// Four tests of the same property run in parallel: each case is a server and a WM
// (about 0.17 s), and cases within one test run one after the other.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(32)))]

    #[test]
    fn workspace_histories_match_the_model_1(ops in prop::collection::vec(op(), 1..24)) {
        history(&ops)?;
    }

    #[test]
    fn workspace_histories_match_the_model_2(ops in prop::collection::vec(op(), 1..24)) {
        history(&ops)?;
    }

    #[test]
    fn workspace_histories_match_the_model_3(ops in prop::collection::vec(op(), 1..24)) {
        history(&ops)?;
    }

    #[test]
    fn workspace_histories_match_the_model_4(ops in prop::collection::vec(op(), 1..24)) {
        history(&ops)?;
    }
}
