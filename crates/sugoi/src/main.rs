use std::path::PathBuf;
use std::process::ExitCode;

use sugoi::Wm;
use sugoi::ruby::RubyScript;

/// [L1] `--config <path>`, or `$XDG_CONFIG_HOME/sugoi/config.rb`.
fn config_path() -> Result<PathBuf, String> {
    let mut args = std::env::args_os().skip(1);
    match (args.next(), args.next(), args.next()) {
        (None, ..) => {}
        (Some(flag), Some(path), None) if flag == "--config" => return Ok(path.into()),
        _ => return Err("usage: sugoi [--config <path>]".into()),
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or("neither XDG_CONFIG_HOME nor HOME is set")?;
    Ok(base.join("sugoi").join("config.rb"))
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let path = match config_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("sugoi: {e}");
            return ExitCode::from(2);
        }
    };
    // SAFETY: called once, on the main thread, before any other Ruby call; `_cleanup`
    // stops Ruby when main returns.
    let _cleanup = unsafe { magnus::embed::init() };
    let ruby = magnus::Ruby::get().expect("Ruby runs on this thread");
    let result = RubyScript::new(&ruby, path)
        .map_err(|e| format!("cannot load the config DSL: {e}"))
        .and_then(|script| Wm::connect(None, Box::new(script)).map_err(|e| e.to_string()))
        .and_then(|mut wm| {
            // Before the settings and windows: a signal must not kill sugoi midway.
            wm.quit_on_signals().and_then(|()| wm.start()).map_err(|e| e.to_string())?;
            wm.run().map_err(|e| e.to_string())
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            eprintln!("sugoi: {e}");
            ExitCode::FAILURE
        }
    }
}
