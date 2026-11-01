//! Where windows go: layouts, states, stacking and mouse drags (`doc/spec/layouts.md`).

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use super::{NORMAL_STATE, Wm, lost};
use crate::layout::{self, Layout, MAX_FACTOR, MIN_FACTOR, Rect};
use crate::{Choice, Error, State};

/// `IconicState` in `WM_STATE` (ICCCM 4.1.3.1).
const ICONIC_STATE: u32 = 3;

/// [G1] [G2] A mouse drag in progress.
pub(super) struct Drag {
    window: Window,
    resize: bool,
    /// Where the pointer started, and the window's rectangle then.
    pointer: (i16, i16),
    rect: Rect,
}

impl Drag {
    pub(super) fn window(&self) -> Window {
        self.window
    }
}

impl Wm {
    /// [Y1] Whether the layout places `window`.
    pub(super) fn tiled(&self, window: Window) -> bool {
        let c = &self.clients[&window];
        self.layout != Layout::Float && !(c.floating || c.maximized || c.fullscreen || c.minimized)
    }

    /// [Y1] The windows the layout places, in the window order.
    pub(super) fn tiled_windows(&self) -> Vec<Window> {
        self.order.iter().copied().filter(|&w| self.tiled(w)).collect()
    }

    /// [W8] The stacking layer of `window`, from the bottom.
    fn layer(&self, window: Window) -> u8 {
        let c = &self.clients[&window];
        if c.fullscreen {
            3
        } else if c.on_top {
            2
        } else if c.floating || c.maximized {
            1
        } else {
            0
        }
    }

    /// [Y1] Places every window where its layout and state say, then restacks.
    pub(super) fn arrange(&mut self) -> Result<(), Error> {
        let tiled = self.tiled_windows();
        let cells = match self.layout {
            Layout::Tile if !tiled.is_empty() => {
                // [Y3] Windows come and go from the stack: keep its weights balanced.
                let mut shares: Vec<f32> =
                    tiled[1..].iter().map(|w| self.clients[w].share).collect();
                layout::balance(&mut shares);
                for (window, &share) in tiled[1..].iter().zip(&shares) {
                    self.clients.get_mut(window).expect("tiled windows are managed").share = share;
                }
                layout::tile(self.screen, self.master, &shares)
            }
            Layout::Max => vec![self.screen; tiled.len()],
            _ => Vec::new(),
        };
        let mut moved = false;
        for (&window, &cell) in tiled.iter().zip(&cells) {
            moved |= self.place(window, cell, self.border.width)?;
        }
        let others: Vec<Window> = self.order.iter().copied().filter(|&w| !self.tiled(w)).collect();
        for window in others {
            let c = &self.clients[&window];
            let (rect, border) = if c.minimized {
                continue;
            } else if c.fullscreen {
                (self.screen, 0)
            } else if c.maximized {
                (self.screen, self.border.width)
            } else {
                (c.rect, self.border.width)
            };
            moved |= self.place(window, rect, border)?;
        }
        moved |= self.restack()?;
        if moved {
            self.ignore_crossings()?;
        }
        Ok(())
    }

    /// Configures `window` to the outer rectangle `cell` with `border`, if it is not
    /// there yet; returns whether it moved.
    fn place(&mut self, window: Window, cell: Rect, border: u16) -> Result<bool, Error> {
        let client = self.clients.get_mut(&window).expect("placed windows are managed");
        if client.placed == Some((cell, border)) {
            return Ok(false);
        }
        client.placed = Some((cell, border));
        let (x, y, width, height) = layout::inner(cell, border);
        let aux = ConfigureWindowAux::new()
            .x(x)
            .y(y)
            .width(width)
            .height(height)
            .border_width(u32::from(border));
        self.conn.configure_window(window, &aux)?;
        Ok(true)
    }

    /// The rectangle `window` was last placed at.
    pub(super) fn placed(&self, window: Window) -> Rect {
        let c = &self.clients[&window];
        c.placed.map_or(c.rect, |(rect, _)| rect)
    }

    /// [W8] Stacks the managed windows by layer, then by the order they were raised;
    /// returns whether anything moved.
    pub(super) fn restack(&mut self) -> Result<bool, Error> {
        let mut stack = self.stack.clone();
        stack.retain(|w| self.clients.get(w).is_some_and(|c| !c.minimized));
        stack.sort_by_key(|&w| self.layer(w));
        let same = stack.iter().zip(&self.stacked).take_while(|(a, b)| a == b).count();
        if same == stack.len() && same == self.stacked.len() {
            return Ok(false);
        }
        for pair in stack[same.saturating_sub(1)..].windows(2) {
            let aux = ConfigureWindowAux::new().sibling(pair[0]).stack_mode(StackMode::ABOVE);
            self.conn.configure_window(pair[1], &aux)?;
        }
        self.stacked = stack;
        Ok(true)
    }

    /// Puts `window` above the others of its layer.
    pub(super) fn raise(&mut self, window: Window) -> Result<(), Error> {
        self.stack.retain(|&w| w != window);
        self.stack.push(window);
        if self.restack()? {
            self.ignore_crossings()?;
        }
        Ok(())
    }

    /// [Z1] Drops the crossing events sugoi's own moves caused: a round trip, then every
    /// queued event but those is kept for the event loop.
    fn ignore_crossings(&mut self) -> Result<(), Error> {
        self.conn.sync().map_err(lost)?;
        while let Some(event) = self.conn.poll_for_event()? {
            if !matches!(event, Event::EnterNotify(_)) {
                self.pending.push_back(event);
            }
        }
        Ok(())
    }

    /// [Y7]
    pub(super) fn choose_layout(&mut self, choice: Choice) -> Result<(), Error> {
        let len = self.layouts.len();
        let index = self.layouts.iter().position(|&l| l == self.layout).unwrap_or(0);
        let layout = match choice {
            Choice::Next => self.layouts[(index + 1) % len],
            Choice::Previous => self.layouts[(index + len - 1) % len],
            Choice::Pick(layout) => layout,
        };
        // [Y6] Windows the layout placed float from where they are.
        if layout == Layout::Float {
            for window in self.tiled_windows() {
                let rect = self.placed(window);
                self.clients.get_mut(&window).expect("managed").rect = rect;
            }
        }
        self.layout = layout;
        self.arrange()
    }

    /// [Y4]
    pub(super) fn master_width(&mut self, delta: f32) -> Result<(), Error> {
        self.master.width = (self.master.width + delta).clamp(MIN_FACTOR, MAX_FACTOR);
        self.arrange()
    }

    /// [Y3] Only a window in the stack column has a share to change.
    pub(super) fn window_height(&mut self, delta: f32) -> Result<(), Error> {
        let tiled = self.tiled_windows();
        let Some(index) = self.focused.and_then(|f| tiled.iter().position(|&w| w == f)) else {
            return Ok(());
        };
        if index == 0 {
            return Ok(());
        }
        let mut shares: Vec<f32> = tiled[1..].iter().map(|w| self.clients[w].share).collect();
        layout::grow_share(&mut shares, index - 1, delta);
        for (window, share) in tiled[1..].iter().zip(shares) {
            self.clients.get_mut(window).expect("tiled windows are managed").share = share;
        }
        self.arrange()
    }

    /// Takes a tiled window out of the layout where it is ([W1], [W7], [G1]).
    /// A tiled, maximized or fullscreen window floats where it shows ([W7], [G1]).
    fn float(&mut self, window: Window) {
        let tiled = self.tiled(window);
        let rect = self.placed(window);
        let client = self.clients.get_mut(&window).expect("managed");
        if tiled || client.maximized || client.fullscreen {
            (client.maximized, client.fullscreen) = (false, false);
            client.floating = true;
            client.rect = rect;
        }
    }

    /// [W1] to [W5]
    pub(super) fn toggle(&mut self, state: State) -> Result<(), Error> {
        let Some(window) = self.focused else { return Ok(()) };
        if state == State::Floating {
            if self.tiled(window) {
                self.float(window);
            } else if self.clients[&window].floating {
                self.clients.get_mut(&window).expect("managed").floating = false;
            }
            return self.arrange();
        }
        let client = self.clients.get_mut(&window).expect("managed");
        let flag = match state {
            State::Maximized => &mut client.maximized,
            State::Fullscreen => &mut client.fullscreen,
            State::OnTop => &mut client.on_top,
            State::Floating => unreachable!(),
        };
        *flag = !*flag;
        self.arrange()
    }

    /// [W7]
    pub(super) fn center(&mut self, size: Option<f32>) -> Result<(), Error> {
        let Some(window) = self.focused else { return Ok(()) };
        self.float(window);
        let screen = self.screen;
        let client = self.clients.get_mut(&window).expect("managed");
        if let Some(size) = size {
            client.rect.width = (screen.width as f32 * size).round() as u32;
            client.rect.height = (screen.height as f32 * size).round() as u32;
        }
        client.rect = layout::center(screen, client.rect);
        self.arrange()
    }

    /// [W6]
    pub(super) fn minimize(&mut self) -> Result<(), Error> {
        let Some(window) = self.focused else { return Ok(()) };
        let client = self.clients.get_mut(&window).expect("managed");
        client.minimized = true;
        client.unmaps += 1;
        self.minimized.push(window);
        self.conn.unmap_window(window)?;
        self.set_state(window, ICONIC_STATE)?;
        self.arrange()?;
        self.refocus(window)
    }

    /// [W6] The most recently minimized window.
    pub(super) fn restore(&mut self) -> Result<(), Error> {
        match self.minimized.last() {
            Some(&window) => self.restore_window(window),
            None => Ok(()),
        }
    }

    pub(super) fn restore_window(&mut self, window: Window) -> Result<(), Error> {
        self.minimized.retain(|&w| w != window);
        self.clients.get_mut(&window).expect("minimized windows are managed").minimized = false;
        self.set_state(window, NORMAL_STATE)?;
        self.arrange()?;
        self.conn.map_window(window)?;
        self.focus(window, true)
    }

    /// After a failed restart: gives the windows sugoi's borders back, and hides the
    /// minimized ones `release_all` mapped.
    pub(super) fn reclaim(&mut self) -> Result<(), Error> {
        for &window in &self.minimized {
            self.clients.get_mut(&window).expect("minimized windows are managed").unmaps += 1;
            self.conn.unmap_window(window)?;
            self.set_state(window, ICONIC_STATE)?;
        }
        for client in self.clients.values_mut() {
            client.placed = None;
        }
        self.regrab(true)?;
        self.arrange()
    }

    pub(super) fn set_state(&self, window: Window, state: u32) -> Result<(), Error> {
        let atom = self.atoms.WM_STATE;
        self.conn.change_property32(PropMode::REPLACE, window, atom, atom, &[state, NONE])?;
        Ok(())
    }

    /// [G1] [G2] Starts dragging the window the last button press hit.
    /// The press is the one being handled: a `move` from a key binding finds none.
    pub(super) fn start_drag(&mut self, resize: bool) -> Result<(), Error> {
        let Some((x, y, Some(window))) = self.press.take() else { return Ok(()) };
        if !self.clients.get(&window).is_some_and(|c| !c.minimized) {
            return Ok(());
        }
        // Grab first: if another client holds the pointer, nothing changes.
        let mask = EventMask::POINTER_MOTION | EventMask::BUTTON_RELEASE;
        let (root, async_) = (self.root, GrabMode::ASYNC);
        let grab =
            self.conn.grab_pointer(false, root, mask, async_, async_, NONE, NONE, CURRENT_TIME);
        if grab?.reply().map_err(lost)?.status != GrabStatus::SUCCESS {
            return Ok(());
        }
        self.float(window);
        self.arrange()?;
        self.raise(window)?;
        let rect = self.clients[&window].rect;
        let pointer = if resize {
            // The corner, kept on screen, where X would clamp the warp anyway.
            let screen = self.screen;
            let right = rect.x + rect.width as i32 - 1;
            let bottom = rect.y + rect.height as i32 - 1;
            let right = right.min(screen.x + screen.width as i32 - 1).max(screen.x);
            let bottom = bottom.min(screen.y + screen.height as i32 - 1).max(screen.y);
            let corner = (right as i16, bottom as i16);
            self.conn.warp_pointer(NONE, self.root, 0, 0, 0, 0, corner.0, corner.1)?;
            corner
        } else {
            (x, y)
        };
        self.drag = Some(Drag { window, resize, pointer, rect });
        Ok(())
    }

    /// [G1] [G2]
    pub(super) fn drag_to(&mut self, x: i16, y: i16) -> Result<(), Error> {
        let Some(drag) = &self.drag else { return Ok(()) };
        let (dx, dy) = (i32::from(x - drag.pointer.0), i32::from(y - drag.pointer.1));
        let mut rect = drag.rect;
        if drag.resize {
            let least = 2 * u32::from(self.border.width) + 1;
            rect.width = (rect.width as i32 + dx).max(least as i32) as u32;
            rect.height = (rect.height as i32 + dy).max(least as i32) as u32;
        } else {
            rect.x += dx;
            rect.y += dy;
        }
        let window = drag.window;
        // A key may have changed the window's state during the drag: it wins.
        let floating = self
            .clients
            .get(&window)
            .is_some_and(|c| !self.tiled(window) && !(c.maximized || c.fullscreen || c.minimized));
        if !floating {
            return self.end_drag();
        }
        self.clients.get_mut(&window).expect("managed").rect = rect;
        // Only the dragged window moves: no arrange, no round trip per motion.
        self.place(window, rect, self.border.width)?;
        Ok(())
    }

    pub(super) fn end_drag(&mut self) -> Result<(), Error> {
        if self.drag.take().is_some() {
            self.conn.ungrab_pointer(CURRENT_TIME)?;
        }
        Ok(())
    }

    /// [W2] Where a window that starts floating goes.
    pub(super) fn initial_rect(&self, rect: Rect, positioned: bool) -> Rect {
        let rect = if positioned { rect } else { layout::center(self.screen, rect) };
        layout::on_screen(self.screen, rect)
    }

    /// [Z2] [F5] Gives the focus away from `gone`: to the window under the pointer, else
    /// the most recently focused one, else the root.
    pub(super) fn refocus(&mut self, gone: Window) -> Result<(), Error> {
        if self.focused != Some(gone) {
            return Ok(());
        }
        self.focused = None;
        let pointer = self.conn.query_pointer(self.root)?.reply().map_err(lost)?.child;
        let next = Some(pointer)
            .filter(|&w| w != gone && self.can_focus(w))
            .or_else(|| self.history.iter().copied().find(|&w| w != gone && self.can_focus(w)));
        match next {
            Some(next) => self.focus(next, false),
            None => {
                let root = super::POINTER_ROOT;
                self.conn.set_input_focus(InputFocus::POINTER_ROOT, root, CURRENT_TIME)?;
                Ok(())
            }
        }
    }

    /// Whether `window` is managed, shown, and takes the focus.
    pub(super) fn can_focus(&self, window: Window) -> bool {
        self.clients.get(&window).is_some_and(|c| c.hints.focusable() && !c.minimized)
    }
}
