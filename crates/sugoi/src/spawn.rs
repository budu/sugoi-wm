//! Starting programs ([P1]) and notifications ([L4], [R1]).

use std::process::{Command, Stdio};

use log::{info, warn};

use crate::Limits;

/// [P1] [A1] Starts `argv` in its own systemd scope under `app-<name>.slice`.
pub fn spawn(name: &str, argv: &[String], limits: &Limits) {
    info!("spawn {name}: {argv:?} {limits:?}");
    let slice = format!("--slice=app-{}.slice", unit_name(name));
    let mut command: Vec<String> = ["systemd-run", "--user", "--scope", "--collect", "--quiet"]
        .into_iter()
        .map(String::from)
        .collect();
    command.push(slice);
    if let Some(memory) = &limits.memory {
        command.extend(["-p".into(), format!("MemoryMax={memory}")]);
    }
    if let Some(weight) = limits.cpu_weight {
        command.extend(["-p".into(), format!("CPUWeight={weight}")]);
    }
    command.push("--".into());
    command.extend(argv.iter().cloned());
    start(command.iter().map(String::as_str));
}

/// [A4] Kills every process of a systemd user unit; false if `systemctl` fails. It runs
/// in the foreground, unlike apps, so its result is known (a few milliseconds).
pub fn kill_unit(unit: &str) -> bool {
    info!("kill {unit}");
    let status = Command::new("systemctl")
        .args(["--user", "kill", "--signal=SIGKILL", unit])
        .stdin(Stdio::null())
        .status();
    match status {
        Ok(status) if status.success() => true,
        Ok(status) => {
            warn!("cannot kill {unit}: systemctl {status}");
            false
        }
        Err(e) => {
            warn!("cannot kill {unit}: {e}");
            false
        }
    }
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
