//! Windows' processes and systemd units (`doc/spec/apps.md`).

use x11rb::protocol::xproto::Window;

use super::Wm;
use crate::Error;
use crate::spawn::kill_unit;

/// [A3] The cgroup v2 path of process `pid` (`/user.slice/.../run-p1-i2.scope`).
pub(super) fn cgroup(pid: &str) -> Option<String> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    text.lines().find_map(|line| line.strip_prefix("0::")).map(str::to_owned)
}

/// [A3] A unit is the last part of a cgroup path.
fn unit(path: &str) -> Option<String> {
    path.rsplit('/').next().filter(|name| !name.is_empty()).map(str::to_owned)
}

/// [A4] Whether `path` is a scope inside the user manager's `app.slice`.
fn app_scope(path: &str) -> bool {
    let in_app_slice =
        path.split('/').skip_while(|p| !p.starts_with("user@")).nth(1) == Some("app.slice");
    in_app_slice && path.ends_with(".scope")
}

/// [A4] The unit `kill` may kill for a process in cgroup `path`: a scope in the user
/// manager's `app.slice`, but never `own`, the cgroup sugoi runs in.
fn killable(path: &str, own: Option<&str>) -> Option<String> {
    (app_scope(path) && Some(path) != own).then(|| unit(path)).flatten()
}

/// [A4] When process `pid` started, in clock ticks since boot (`/proc/<pid>/stat`'s 22nd
/// field), which tells it from a later process given the same ID.
pub(super) fn start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name, in parentheses, may hold spaces: count fields after it.
    let rest = &stat[stat.rfind(')')? + 2..];
    rest.split(' ').nth(19)?.parse().ok()
}

/// [A3] The unit process `pid` is in.
pub(super) fn unit_of(pid: u32) -> Option<String> {
    unit(&cgroup(&pid.to_string())?)
}

impl Wm {
    /// [A4] Kills the unit of `window`'s app if that is safe, else closes the window. The
    /// process must be the one seen at manage: the one that opened the connection may
    /// have exited, and its ID gone to another app.
    pub(super) fn kill(&mut self, window: Window) -> Result<(), Error> {
        let pid = self.clients.get(&window).and_then(|c| {
            let pid = c.info.pid?;
            (c.started.is_some() && start_time(pid) == c.started).then_some(pid)
        });
        let path = pid.and_then(|p| cgroup(&p.to_string()));
        match path.and_then(|path| killable(&path, self.own_cgroup.as_deref())) {
            Some(unit) if kill_unit(&unit) => Ok(()),
            _ => self.close(window),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_the_last_part_of_the_path() {
        let path = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-kitty.slice/run-p5-i8.scope";
        assert_eq!(unit(path).as_deref(), Some("run-p5-i8.scope"));
        assert!(app_scope(path));
    }

    #[test]
    fn only_app_scopes_are_killable() {
        // The X session, where the WM itself runs.
        assert!(!app_scope("/user.slice/user-1000.slice/session-3.scope"));
        // A service of the user manager, and a slice.
        assert!(!app_scope(
            "/user.slice/user-1000.slice/user@1000.service/app.slice/pipewire.service"
        ));
        assert!(!app_scope("/user.slice/user-1000.slice/user@1000.service/app.slice"));
        assert!(!app_scope("/user.slice/user-1000.slice/user@1000.service/session.slice/x.scope"));
        let kitty_child = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-superkitty.slice/kitty-21404-5.scope";
        assert!(app_scope(kitty_child));
    }

    #[test]
    fn start_times_tell_processes_apart() {
        let me = std::process::id();
        let started = start_time(me);
        assert!(started.is_some());
        assert_eq!(start_time(me), started);
        assert_eq!(start_time(u32::MAX), None);
    }

    #[test]
    fn sugois_own_scope_is_never_killable() {
        let emacs = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-emacs.slice/run-p1-i2.scope";
        assert_eq!(killable(emacs, None).as_deref(), Some("run-p1-i2.scope"));
        assert_eq!(killable(emacs, Some(emacs)), None);
    }
}
