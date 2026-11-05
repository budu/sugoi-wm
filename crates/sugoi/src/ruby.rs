//! The Ruby config ([L1], [L2]): CRuby embedded through magnus. Ruby must run on the
//! thread that started it, so this is only for the binary; tests drive it through it.

use std::path::PathBuf;

use magnus::prelude::*;
use magnus::{ArgList, RModule, RString, Ruby, TryConvert, Value};

use crate::{
    Binding, Border, Choice, Command, Direction, Effects, Event, Goto, HookArg, Layout, Limits,
    Master, Matcher, Rule, Script, Settings, Side, Snapshot, State, View, WindowCommand,
    WindowType,
};

pub struct RubyScript {
    path: PathBuf,
}

type Chords = Vec<(String, usize)>;
/// What `Sugoi.load` exports: border width and colors, modkey, keys, buttons, root
/// buttons, layouts, master side and width.
type Exported = (
    (Option<u16>, Option<u32>, Option<u32>, Option<u32>, Option<u32>),
    String,
    Chords,
    Chords,
    Chords,
    Vec<String>,
    Option<String>,
    Option<f32>,
    Vec<RuleExport>,
    Vec<(String, usize)>,
);
/// A rule as `Sugoi::Config#rule` exports it: class, instance, title, types, transient,
/// float, size, center, on top, master, workspace.
type RuleExport = (
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Option<bool>,
    Option<bool>,
    Option<(u32, u32)>,
    bool,
    Option<bool>,
    bool,
    Option<u32>,
);
/// [H2] [A5] A window for `Sugoi.run_hook`: id, class, instance, title, type, transient,
/// workspace, floating, pid, unit, minimized.
type WindowExport =
    (u32, String, String, String, String, bool, u32, bool, Option<u32>, Option<String>, bool);

fn window_export(s: &Snapshot) -> WindowExport {
    let i = &s.info;
    let kind = i.kind().name().to_owned();
    (
        s.id,
        i.class.clone(),
        i.instance.clone(),
        i.title.clone(),
        kind,
        i.transient,
        s.workspace,
        s.floating,
        i.pid,
        s.unit.clone(),
        s.minimized,
    )
}

fn rule(r: RuleExport) -> Result<Rule, String> {
    let (class, instance, title, types, transient, float, size, center, on_top, master, workspace) =
        r;
    let types = types
        .iter()
        .map(|t| WindowType::from_name(t).ok_or_else(|| format!("bad window type {t:?}")))
        .collect::<Result<_, _>>()?;
    Ok(Rule {
        matcher: Matcher { class, instance, title, types, transient },
        effects: Effects { float, size, center, on_top, master, workspace },
    })
}

impl RubyScript {
    /// Loads the DSL into the Ruby `ruby` proves is running on this thread.
    pub fn new(ruby: &Ruby, path: PathBuf) -> Result<RubyScript, String> {
        ruby.eval::<Value>(include_str!("../ruby/prelude.rb")).map_err(|e| e.to_string())?;
        Ok(RubyScript { path })
    }

    /// Calls the `Sugoi` module function `method`.
    fn call<T: TryConvert>(&self, method: &str, args: impl ArgList) -> Result<T, String> {
        let ruby = Ruby::get().map_err(|e| e.to_string())?;
        let sugoi: RModule = ruby.define_module("Sugoi").map_err(|e| e.to_string())?;
        sugoi.funcall(method, args).map_err(|e| e.to_string())
    }
}

impl Script for RubyScript {
    fn load(&mut self) -> Result<Settings, String> {
        // [L3] No file is not an error: the defaults apply.
        if !self.path.exists() {
            log::info!("no config at {}, using the defaults", self.path.display());
            return Ok(Settings::default());
        }
        let path = self.path.to_string_lossy().into_owned();
        let value: Value = self.call("load", (path,))?;
        if let Some(message) = RString::from_value(value) {
            return Err(message.to_string().map_err(|e| e.to_string())?);
        }
        let (
            (width, focused, normal, on_top, focused_on_top),
            modkey,
            keys,
            buttons,
            root_buttons,
            layouts,
            side,
            share,
            rules,
            hooks,
        ) = Exported::try_convert(value).map_err(|e| e.to_string())?;
        let defaults = Border::default();
        let bind = |list: Vec<(String, usize)>| {
            list.into_iter()
                .map(|(chord, block)| Binding { chord, action: crate::Action::Script(block) })
                .collect()
        };
        Ok(Settings {
            border: Border {
                width: width.unwrap_or(defaults.width),
                focused: focused.unwrap_or(defaults.focused),
                normal: normal.unwrap_or(defaults.normal),
                on_top: on_top.unwrap_or(defaults.on_top),
                focused_on_top: focused_on_top.unwrap_or(defaults.focused_on_top),
            },
            modkey,
            keys: bind(keys),
            buttons: bind(buttons),
            root_buttons: bind(root_buttons),
            rules: rules.into_iter().map(rule).collect::<Result<_, _>>()?,
            hooks: hooks
                .into_iter()
                .map(|(event, block)| {
                    let event = match event.as_str() {
                        "manage" => Event::Manage,
                        "focus" => Event::Focus,
                        "unmanage" => Event::Unmanage,
                        "workspace" => Event::Workspace,
                        other => return Err(format!("bad hook event {other:?}")),
                    };
                    Ok((event, block))
                })
                .collect::<Result<_, String>>()?,
            layouts: layouts.iter().map(|l| layout(l)).collect::<Result<_, _>>()?,
            master: Master {
                side: match side.as_deref() {
                    None => Master::default().side,
                    Some("left") => Side::Left,
                    Some("right") => Side::Right,
                    Some(other) => return Err(format!("bad master side {other:?}")),
                },
                width: share.unwrap_or(Master::default().width),
            },
        })
    }

    fn commit(&mut self) {
        if let Err(e) = self.call::<Value>("commit", ()) {
            log::error!("commit: {e}");
        }
    }

    fn run(&mut self, block: usize, view: &View) -> Result<Vec<Command>, String> {
        let windows: Vec<WindowExport> = view.windows.iter().map(window_export).collect();
        let value: Value = self.call("run", (block, windows, view.workspace))?;
        if let Some(message) = RString::from_value(value) {
            return Err(message.to_string().map_err(|e| e.to_string())?);
        }
        let commands = Vec::<Vec<String>>::try_convert(value).map_err(|e| e.to_string())?;
        commands.iter().map(|c| command(c)).collect()
    }

    fn eval(&mut self, code: &str, view: &View) -> Result<(String, Vec<Command>), String> {
        let windows: Vec<WindowExport> = view.windows.iter().map(window_export).collect();
        let value: Value = self.call("eval_request", (code.to_owned(), windows, view.workspace))?;
        if let Some(message) = RString::from_value(value) {
            return Err(message.to_string().map_err(|e| e.to_string())?);
        }
        let (inspected, commands) =
            <(String, Vec<Vec<String>>)>::try_convert(value).map_err(|e| e.to_string())?;
        let commands = commands.iter().map(|c| command(c)).collect::<Result<_, _>>()?;
        Ok((inspected, commands))
    }

    fn hook(&mut self, block: usize, arg: &HookArg, view: &View) -> Result<Vec<Command>, String> {
        let (window, workspace) = match arg {
            HookArg::Window(s) => (Some(window_export(s)), None),
            HookArg::Workspace(n) => (None, Some(*n)),
        };
        let windows: Vec<WindowExport> = view.windows.iter().map(window_export).collect();
        let current = view.workspace;
        let value: Value = self.call("run_hook", (block, window, workspace, windows, current))?;
        if let Some(message) = RString::from_value(value) {
            return Err(message.to_string().map_err(|e| e.to_string())?);
        }
        let commands = Vec::<Vec<String>>::try_convert(value).map_err(|e| e.to_string())?;
        commands.iter().map(|c| command(c)).collect()
    }
}

/// Converts a queued `[name, args...]` into a command.
fn command(parts: &[String]) -> Result<Command, String> {
    let direction = |d: Option<&String>| match d.map(String::as_str) {
        Some("next") => Ok(Direction::Next),
        Some("previous") => Ok(Direction::Previous),
        other => Err(format!("bad direction {other:?}")),
    };
    Ok(match parts {
        [name, rest @ ..] => match (name.as_str(), rest) {
            ("focus", [d]) => Command::Focus(direction(Some(d))?),
            ("swap", [d]) => Command::Swap(direction(Some(d))?),
            ("close", []) => Command::CloseFocused,
            ("reload", []) => Command::Reload,
            ("restart", []) => Command::Restart,
            ("quit", []) => Command::Quit,
            ("spawn", [name, memory, weight, argv @ ..]) if !argv.is_empty() => {
                let some = |s: &String| (!s.is_empty()).then(|| s.clone());
                let cpu_weight = some(weight).map(|w| w.parse()).transpose();
                let cpu_weight = cpu_weight.map_err(|_| format!("bad cpu_weight {weight:?}"))?;
                let limits = Limits { memory: some(memory), cpu_weight };
                Command::Spawn { name: name.clone(), argv: argv.to_vec(), limits }
            }
            ("kill", []) => Command::Kill,
            ("layout", [choice]) => Command::Layout(match choice.as_str() {
                "next" => Choice::Next,
                "previous" => Choice::Previous,
                name => Choice::Pick(layout(name)?),
            }),
            ("master_width", [delta]) => Command::MasterWidth(number(delta)?),
            ("window_height", [delta]) => Command::WindowHeight(number(delta)?),
            ("toggle", [state]) => Command::Toggle(match state.as_str() {
                "floating" => State::Floating,
                "maximized" => State::Maximized,
                "fullscreen" => State::Fullscreen,
                "on_top" => State::OnTop,
                other => return Err(format!("bad state {other:?}")),
            }),
            ("minimize", []) => Command::Minimize,
            ("restore", []) => Command::Restore,
            ("center", []) => Command::Center(None),
            ("center", [size]) => Command::Center(Some(number(size)?)),
            ("move", []) => Command::Move,
            ("workspace", [target]) => Command::Workspace(match target.as_str() {
                "next" => Goto::Next,
                "previous" => Goto::Previous,
                "back" => Goto::Back,
                n => Goto::Number(n.parse().map_err(|_| format!("bad workspace {n:?}"))?),
            }),
            ("on", [id, rest @ ..]) => {
                let id = id.parse().map_err(|_| format!("bad window {id:?}"))?;
                let number = |n: &String| n.parse().map_err(|_| format!("bad number {n:?}"));
                Command::On(
                    id,
                    match rest {
                        [a] if a == "float" => WindowCommand::Float(true),
                        [a] if a == "tile" => WindowCommand::Float(false),
                        [a, w, h] if a == "size" => WindowCommand::Size(number(w)?, number(h)?),
                        [a] if a == "center" => WindowCommand::Center,
                        [a] if a == "master" => WindowCommand::Master,
                        [a, n] if a == "move_to" => WindowCommand::MoveTo(number(n)?),
                        [a] if a == "close" => WindowCommand::Close,
                        [a] if a == "kill" => WindowCommand::Kill,
                        [a] if a == "activate" => WindowCommand::Activate,
                        _ => return Err(format!("bad window command {rest:?}")),
                    },
                )
            }
            ("move_to", [n]) => {
                Command::MoveTo(n.parse().map_err(|_| format!("bad workspace {n:?}"))?)
            }
            ("resize", []) => Command::Resize,
            _ => return Err(format!("bad command {parts:?}")),
        },
        [] => return Err("empty command".into()),
    })
}

fn layout(name: &str) -> Result<Layout, String> {
    Layout::from_name(name).ok_or_else(|| format!("bad layout {name:?}"))
}

fn number(text: &str) -> Result<f32, String> {
    text.parse().map_err(|_| format!("bad number {text:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(parts: &[&str]) -> Result<Command, String> {
        command(&parts.iter().map(|p| p.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn converts_the_layout_commands() {
        assert_eq!(parse(&["layout", "next"]), Ok(Command::Layout(Choice::Next)));
        assert_eq!(parse(&["layout", "max"]), Ok(Command::Layout(Choice::Pick(Layout::Max))));
        assert_eq!(parse(&["master_width", "-0.05"]), Ok(Command::MasterWidth(-0.05)));
        assert_eq!(parse(&["window_height", "0.05"]), Ok(Command::WindowHeight(0.05)));
        assert_eq!(parse(&["toggle", "on_top"]), Ok(Command::Toggle(State::OnTop)));
        assert_eq!(parse(&["center"]), Ok(Command::Center(None)));
        assert_eq!(parse(&["center", "0.7"]), Ok(Command::Center(Some(0.7))));
        assert_eq!(parse(&["resize"]), Ok(Command::Resize));
        assert_eq!(parse(&["workspace", "13"]), Ok(Command::Workspace(Goto::Number(13))));
        assert_eq!(parse(&["workspace", "back"]), Ok(Command::Workspace(Goto::Back)));
        assert_eq!(parse(&["move_to", "3"]), Ok(Command::MoveTo(3)));
    }

    #[test]
    fn rejects_bad_commands() {
        assert!(parse(&["layout", "spiral"]).is_err());
        assert!(parse(&["master_width", "lots"]).is_err());
        assert!(parse(&["toggle", "sticky"]).is_err());
    }
}
