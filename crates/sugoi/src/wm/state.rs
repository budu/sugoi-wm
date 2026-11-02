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
//! window 0x1a00003 1 100 80 640 400 1.25
//! ```
//!
//! A window line is its ID, whether it floats, its floating geometry and its share.
//! Lines sugoi does not understand are skipped; another version is ignored whole.

use std::collections::HashMap;

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
}

fn layout_name(layout: Layout) -> &'static str {
    match layout {
        Layout::Tile => "tile",
        Layout::Max => "max",
        Layout::Float => "float",
    }
}

fn layout_named(name: &str) -> Option<Layout> {
    match name {
        "tile" => Some(Layout::Tile),
        "max" => Some(Layout::Max),
        "float" => Some(Layout::Float),
        _ => None,
    }
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
            kept.desks.push((n, layout_named(layout)?, width));
        }
        ["order", ..] => kept.order = list(),
        ["history", ..] => kept.history = list(),
        ["minimized", ..] => kept.minimized = list(),
        ["window", id, floating, x, y, width, height, share] => {
            let (x, y) = (x.parse().ok()?, y.parse().ok()?);
            let rect = Rect::new(x, y, width.parse().ok()?, height.parse().ok()?);
            let share: f32 = share.parse().ok().filter(|s: &f32| s.is_finite() && *s > 0.0)?;
            let floating = match *floating {
                "1" => true,
                "0" => false,
                _ => return None,
            };
            kept.windows.insert(window_id(id)?, KeptWindow { floating, rect, share });
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
            let layout = layout_name(desk.layout);
            text += &format!("desk {n} {layout} {}\n", desk.master.width);
        }
        text += &format!("order{}\n", ids(&self.order));
        text += &format!("history{}\n", ids(&self.history));
        text += &format!("minimized{}\n", ids(&self.minimized));
        for &window in &self.order {
            let c = &self.clients[&window];
            let r = c.rect;
            let floating = u8::from(c.floating);
            text += &format!(
                "window {window:#x} {floating} {} {} {} {} {}\n",
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
        let screen = self.screen;
        for (window, k) in &kept.windows {
            if let Some(client) = self.clients.get_mut(window) {
                client.floating = k.floating;
                client.rect = layout::on_screen(screen, k.rect);
                client.share = k.share;
            }
        }
        for &window in &kept.minimized {
            if let Some(client) = self.clients.get_mut(&window) {
                client.minimized = true;
                self.minimized.push(window);
            }
        }
        // Windows the state names, in its order; the others after, as adopted.
        let reorder = |list: &mut Vec<Window>, saved: &[Window]| {
            let mut ordered: Vec<Window> =
                saved.iter().copied().filter(|w| list.contains(w)).collect();
            ordered.extend(list.iter().copied().filter(|w| !saved.contains(w)));
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
        let window = KeptWindow { floating: true, rect: Rect::new(-5, 80, 640, 400), share: 1.25 };
        assert_eq!(kept.windows[&0x10], window);
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
