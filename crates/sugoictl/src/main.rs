//! `sugoictl <ruby>`: runs Ruby in the running sugoi and prints the reply ([I3]).

use std::process::ExitCode;

use sugoictl::{Reply, default_socket, request};

fn main() -> ExitCode {
    let code = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if code.trim().is_empty() {
        eprintln!("usage: sugoictl <ruby>, e.g. sugoictl \"workspace 3\"");
        return ExitCode::from(2);
    }
    let Some(socket) = default_socket() else {
        eprintln!("sugoictl: no SUGOI_SOCKET, or no DISPLAY and XDG_RUNTIME_DIR");
        return ExitCode::from(2);
    };
    match request(&socket, &code) {
        Ok(Reply::Ok(value)) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Ok(Reply::Error(message)) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("sugoictl: cannot reach sugoi at {}: {e}", socket.display());
            ExitCode::from(2)
        }
    }
}
