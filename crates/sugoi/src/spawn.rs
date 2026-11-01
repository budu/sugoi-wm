//! Starting programs ([P1]) and notifications ([L4], [R1]).

use std::process::{Command, Stdio};

use log::{info, warn};

/// [P1] Starts `argv` in its own systemd scope under `app-<name>.slice`.
pub fn spawn(name: &str, argv: &[String]) {
    info!("spawn {name}: {argv:?}");
    let slice = format!("--slice=app-{}.slice", unit_name(name));
    let systemd_run = ["systemd-run", "--user", "--scope", "--collect", "--quiet", &slice, "--"];
    start(systemd_run.iter().copied().chain(argv.iter().map(String::as_str)));
}

/// Shows `message` through the notification daemon (dunst), if there is one.
pub fn notify(message: &str) {
    start(["notify-send", "--app-name=sugoi", "sugoi", message]);
}

/// Runs `argv` through `setsid -f`, so the program is never sugoi's child: it is reaped
/// across a restart, and gets no signal sent to sugoi's group. `setsid` forks and exits
/// at once, so waiting for it does not block.
fn start<'a>(argv: impl IntoIterator<Item = &'a str>) {
    let mut argv = argv.into_iter();
    let Some(program) = argv.next() else { return };
    // `setsid -f` exits 0 even when the program is missing, so look first.
    if !on_path(program) {
        warn!("cannot start {program}: not found on PATH");
        return;
    }
    let mut command = Command::new("setsid");
    command.arg("-f").arg(program).args(argv).stdin(Stdio::null());
    if let Err(e) = command.spawn().and_then(|mut setsid| setsid.wait()) {
        warn!("cannot start {program}: {e}");
    }
}

fn on_path(program: &str) -> bool {
    let paths = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
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
