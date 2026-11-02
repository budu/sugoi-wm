//! [V11] sugoi's own state, kept in the root's `_SUGOI_STATE` across restarts.
//!
//! The property is text, one record per line, so a person can read it with `xprop`:
//!
//! ```text
//! sugoi-state 1
//! back 3
//! desk 0 tile 0.5
//! order 0x1a00003 0x2200004
//! history 0x2200004 0x1a00003
//! minimized 0x2200004
//! window 0x1a00003 1 100 80 640 400 1.25 5151 81971
//! ```
//!
//! A window line is its ID, whether it floats, its floating geometry, its share, and its
//! process's ID and start time (`-` when unknown, [A2]), so that a window reusing the ID
//! is not taken for it. Lines sugoi does not understand are skipped; another version is
//! ignored whole.

use std::collections::{HashMap, HashSet};

use log::debug;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;

use super::{Wm, lost};
use crate::Error;
use crate::layout::{self, Layout, MAX_FACTOR, MIN_FACTOR, Rect};
use crate::workspace::COUNT;

const HEADER: &str = "sugoi-state 1";

/// The state as read back.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Kept {
    back: Option<usize>,
    desks: Vec<(usize, Layout, f32)>,
    order: Vec<Window>,
    history: Vec<Window>,
    minimized: Vec<Window>,
    windows: HashMap<Window, KeptWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct KeptWindow {
    floating: bool,
    rect: Rect,
    share: f32,
    /// The process's ID and start time; `None` from a line written before they were.
    process: Option<(Option<u32>, Option<u64>)>,
}

/// [V11] Whether a window is the one its line was saved for: the same process, or both
/// unknown; a line from before processes were saved names its window as is.
fn same_process(
    saved: Option<(Option<u32>, Option<u64>)>,
    now: (Option<u32>, Option<u64>),
) -> bool {
    // An unknown process (a remote client) cannot tell: the ID alone decides.
    saved.is_none_or(|saved| saved == now)
}

/// A number, or `-` for none.
fn optional<T: std::str::FromStr>(text: &str) -> Option<Option<T>> {
    if text == "-" { Some(None) } else { text.parse().ok().map(Some) }
}

fn or_dash<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| v.to_string())
}

fn window_id(text: &str) -> Option<Window> {
    Window::from_str_radix(text.strip_prefix("0x")?, 16).ok()
}

fn ids(windows: &[Window]) -> String {
    windows.iter().map(|w| format!(" {w:#x}")).collect()
}

/// Reads the state's text; `None` for another version or no state.
fn parse(text: &str) -> Option<Kept> {
    let mut lines = text.lines();
    if lines.next()? != HEADER {
        return None;
    }
    let mut kept = Kept::default();
    for line in lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        if record(&mut kept, &words).is_none() {
            debug!("kept state: skipping {line:?}");
        }
    }
    Some(kept)
}

/// Reads one line into `kept`; `None` when it is not one sugoi understands.
fn record(kept: &mut Kept, words: &[&str]) -> Option<()> {
    let list = || words[1..].iter().filter_map(|w| window_id(w)).collect::<Vec<_>>();
    match words {
        ["back", n] => kept.back = Some(n.parse().ok().filter(|&n| n < COUNT)?),
        ["desk", n, layout, width] => {
            let n = n.parse().ok().filter(|&n: &usize| n < COUNT)?;
            let width: f32 = width.parse().ok().filter(|w: &f32| w.is_finite())?;
            kept.desks.push((n, Layout::from_name(layout)?, width));
        }
        ["order", ..] => kept.order = list(),
        ["history", ..] => kept.history = list(),
        ["minimized", ..] => kept.minimized = list(),
        ["window", id, floating, x, y, width, height, share, process @ ..] => {
            let process = match process {
                [] => None,
                [pid, start] => Some((optional(pid)?, optional(start)?)),
                _ => return None,
            };
            let (x, y) = (x.parse().ok()?, y.parse().ok()?);
            let rect = Rect::new(x, y, width.parse().ok()?, height.parse().ok()?);
            let share: f32 = share.parse().ok().filter(|s: &f32| s.is_finite() && *s > 0.0)?;
            let floating = match *floating {
                "1" => true,
                "0" => false,
                _ => return None,
            };
            let window = KeptWindow { floating, rect, share, process };
            kept.windows.insert(window_id(id)?, window);
        }
        _ => return None,
    }
    Some(())
}

impl Wm {
    /// [V11] The state as text, in the order the format lists.
    fn state_text(&self) -> String {
        let mut text = format!("{HEADER}\nback {}\n", self.back);
        for (n, desk) in self.desks.iter().enumerate() {
            let layout = desk.layout.name();
            text += &format!("desk {n} {layout} {}\n", desk.master.width);
        }
        text += &format!("order{}\n", ids(&self.order));
        text += &format!("history{}\n", ids(&self.history));
        text += &format!("minimized{}\n", ids(&self.minimized));
        for &window in &self.order {
            let c = &self.clients[&window];
            let r = c.rect;
            let floating = u8::from(c.floating);
            let (pid, start) = (or_dash(c.info.pid), or_dash(c.started));
            text += &format!(
                "window {window:#x} {floating} {} {} {} {} {} {pid} {start}\n",
                r.x, r.y, r.width, r.height, c.share
            );
        }
        text
    }

    /// [V11] Writes the state when it changed since it was last written.
    pub(super) fn save_state(&mut self) -> Result<(), Error> {
        let text = self.state_text();
        if self.saved_state.as_deref() != Some(text.as_str()) {
            let (property, utf8) = (self.atoms._SUGOI_STATE, self.atoms.UTF8_STRING);
            self.conn.change_property8(
                PropMode::REPLACE,
                self.root,
                property,
                utf8,
                text.as_bytes(),
            )?;
            self.saved_state = Some(text);
        }
        Ok(())
    }

    /// [V11] The state a previous sugoi left, if any.
    pub(super) fn read_state(&self) -> Result<Option<Kept>, Error> {
        let (property, utf8) = (self.atoms._SUGOI_STATE, self.atoms.UTF8_STRING);
        let reply = self.conn.get_property(false, self.root, property, utf8, 0, u32::MAX / 4)?;
        let reply = reply.reply().map_err(lost)?;
        Ok(std::str::from_utf8(&reply.value).ok().and_then(parse))
    }

    /// [V11] Gives the adopted windows and the workspaces their kept state back, before
    /// the first arrange; rules have run, and the state wins over them.
    pub(super) fn restore_state(&mut self, kept: Kept) {
        if let Some(back) = kept.back {
            self.back = back;
        }
        for (n, layout, width) in kept.desks {
            let desk = &mut self.desks[n];
            if self.layouts.contains(&layout) {
                desk.layout = layout;
            }
            desk.master.width = width.clamp(MIN_FACTOR, MAX_FACTOR);
        }
        // Only the windows that are the ones the state names: same ID, same process.
        let known: HashSet<Window> = kept
            .windows
            .iter()
            .filter(|(window, k)| {
                self.clients
                    .get(window)
                    .is_some_and(|c| same_process(k.process, (c.info.pid, c.started)))
            })
            .map(|(&window, _)| window)
            .collect();
        let screen = self.screen;
        for window in &known {
            let (k, client) = (kept.windows[window], self.clients.get_mut(window).expect("known"));
            client.floating = k.floating;
            client.rect = layout::on_screen(screen, k.rect);
            client.share = k.share;
        }
        for window in kept.minimized.iter().filter(|w| known.contains(w)) {
            let client = self.clients.get_mut(window).expect("known");
            // A name repeated in the state is the same window.
            if !client.minimized {
                client.minimized = true;
                self.minimized.push(*window);
            }
        }
        // Windows the state names, once each, in its order; the others after, as adopted.
        let reorder = |list: &mut Vec<Window>, saved: &[Window]| {
            let mut seen = HashSet::new();
            let mut ordered: Vec<Window> =
                saved.iter().copied().filter(|w| known.contains(w) && seen.insert(*w)).collect();
            ordered.extend(list.iter().copied().filter(|w| !seen.contains(w)));
            *list = ordered;
        };
        reorder(&mut self.order, &kept.order);
        reorder(&mut self.history, &kept.history);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_reads_back() {
        let text = "sugoi-state 1\nback 3\ndesk 4 max 0.62\norder 0x10 0x20\n\
                    history 0x20 0x10\nminimized 0x20\n\
                    window 0x10 1 -5 80 640 400 1.25\n";
        let kept = parse(text).unwrap();
        assert_eq!(kept.back, Some(3));
        assert_eq!(kept.desks, [(4, Layout::Max, 0.62)]);
        assert_eq!(kept.order, [0x10, 0x20]);
        assert_eq!(kept.history, [0x20, 0x10]);
        assert_eq!(kept.minimized, [0x20]);
        let rect = Rect::new(-5, 80, 640, 400);
        let window = KeptWindow { floating: true, rect, share: 1.25, process: None };
        assert_eq!(kept.windows[&0x10], window);
    }

    #[test]
    fn a_window_line_keeps_its_process_or_dashes() {
        let text = "sugoi-state 1\nwindow 0x10 0 0 0 9 9 1 5151 81971\n\
                    window 0x20 0 0 0 9 9 1 - -\nwindow 0x30 0 0 0 9 9 1 5151\n";
        let kept = parse(text).unwrap();
        assert_eq!(kept.windows[&0x10].process, Some((Some(5151), Some(81971))));
        assert_eq!(kept.windows[&0x20].process, Some((None, None)));
        assert!(!kept.windows.contains_key(&0x30));
    }

    #[test]
    fn only_a_known_and_equal_process_is_the_same() {
        let now = (Some(5151), Some(81971));
        assert!(same_process(Some(now), now));
        assert!(same_process(None, now));
        assert!(!same_process(Some((Some(5151), Some(1))), now));
        assert!(same_process(Some((None, None)), (None, None)));
        assert!(!same_process(Some((None, None)), now));
    }

    #[test]
    fn bad_lines_are_skipped_and_another_version_ignored() {
        let text = "sugoi-state 1\nback 99\ndesk 2 spiral 0.5\nwindow 0x10 2 0 0 1 1 1\n\
                    window 0x20 0 0 0 1 1 NaN\nlater 1\norder 0x10 nope 0x30\n";
        let kept = parse(text).unwrap();
        assert_eq!(kept.back, None);
        assert!(kept.desks.is_empty() && kept.windows.is_empty());
        assert_eq!(kept.order, [0x10, 0x30]);
        assert_eq!(parse("sugoi-state 2\nback 3\n"), None);
        assert_eq!(parse(""), None);
    }
}
