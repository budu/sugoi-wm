use std::process::ExitCode;

use sugoi::{Config, Wm};

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let result = Wm::new(None, Config::default()).and_then(Wm::run);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            eprintln!("sugoi: {e}");
            ExitCode::FAILURE
        }
    }
}
