//! Starting programs ([P1]) and notifications ([L4], [R1]).

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;

use log::{info, warn};

/// [P1] Starts `argv` in its own systemd scope under `app-<name>.slice`.
pub fn spawn(name: &str, argv: &[String]) {
    info!("spawn {name}: {argv:?}");
    let slice = format!("--slice=app-{}.slice", unit_name(name));
    let mut command = Command::new("systemd-run");
    command.args(["--user", "--scope", "--collect", "--quiet", &slice, "--"]).args(argv);
    start(command);
}

/// Shows `message` through the notification daemon (dunst), if there is one.
pub fn notify(message: &str) {
    let mut command = Command::new("notify-send");
    command.args(["--app-name=sugoi", "sugoi", message]);
    start(command);
}

/// Runs `command` detached from sugoi's process group, and reaps it in a thread.
fn start(mut command: Command) {
    command.stdin(Stdio::null()).process_group(0);
    match command.spawn() {
        Ok(mut child) => {
            thread::spawn(move || child.wait());
        }
        Err(e) => warn!("cannot start {:?}: {e}", command.get_program()),
    }
}

/// Keeps a unit name to letters, digits and `_`: a `-` would nest the slice.
fn unit_name(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn unit_names_keep_slices_flat() {
        assert_eq!(super::unit_name("kitty"), "kitty");
        assert_eq!(super::unit_name("zed-editor 2"), "zed_editor_2");
    }
}
