//! The Ruby config ([L1], [L2]): CRuby embedded through magnus. Ruby must run on the
//! thread that started it, so this is only for the binary; tests drive it through it.

use std::path::{Path, PathBuf};

use magnus::prelude::*;
use magnus::{ArgList, RModule, RString, Ruby, TryConvert, Value};

use crate::{Binding, Border, Command, Direction, Script, Settings};

pub struct RubyScript {
    path: PathBuf,
}

/// What `Sugoi.load` exports: border width and colors, modkey, keys, buttons.
type Exported =
    (Option<u16>, Option<u32>, Option<u32>, String, Vec<(String, usize)>, Vec<(String, usize)>);

impl RubyScript {
    /// Loads the DSL into the Ruby `ruby` proves is running on this thread.
    pub fn new(ruby: &Ruby, path: PathBuf) -> Result<RubyScript, String> {
        ruby.eval::<Value>(include_str!("../ruby/prelude.rb")).map_err(|e| e.to_string())?;
        Ok(RubyScript { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
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
        let (width, focused, normal, modkey, keys, buttons) =
            Exported::try_convert(value).map_err(|e| e.to_string())?;
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
            },
            modkey,
            keys: bind(keys),
            buttons: bind(buttons),
        })
    }

    fn commit(&mut self) {
        if let Err(e) = self.call::<Value>("commit", ()) {
            log::error!("commit: {e}");
        }
    }

    fn run(&mut self, block: usize) -> Result<Vec<Command>, String> {
        let value: Value = self.call("run", (block,))?;
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
            ("spawn", [name, argv @ ..]) if !argv.is_empty() => {
                Command::Spawn { name: name.clone(), argv: argv.to_vec() }
            }
            _ => return Err(format!("bad command {parts:?}")),
        },
        [] => return Err("empty command".into()),
    })
}
