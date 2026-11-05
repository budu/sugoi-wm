//! Where windows go: layouts, states, stacking and mouse drags (`doc/spec/layouts.md`).

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use super::{Wm, lost};
use crate::layout::{self, Layout, MAX_FACTOR, MIN_FACTOR, Rect};
use crate::{Choice, Error, State};

/// [G1] [G2] [G3] A mouse drag in progress.
pub(super) struct Drag {
    window: Window,
    kind: Kind,
    /// Where the pointer started, and the window's rectangle then.
    pointer: (i16, i16),
    rect: Rect,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// [G1] A floating window follows the pointer.
    Move,
    /// [G2] From a corner; the opposite one stays.
    Resize { left: bool, top: bool },
    /// [G1] A tiled window trades places with the tiled windows the pointer enters.
    Swap,
    /// [G3] The master width and the window's edge follow the pointer.
    Layout,
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
        self.desks[c.workspace].layout != Layout::Float
            && !(c.floating || c.maximized || c.fullscreen || c.minimized)
    }

    /// [Y1] The windows the layout places, in the window order.
    pub(super) fn tiled_windows(&self) -> Vec<Window> {
        self.order.iter().copied().filter(|&w| self.visible(w) && self.tiled(w)).collect()
    }

    /// [W8] The stacking layer of `window`, from the bottom: tiled, floating and
    /// maximized windows share the first, in the order they were raised.
    pub(super) fn layer(&self, window: Window) -> u8 {
        let c = &self.clients[&window];
        if c.fullscreen {
            2
        } else if c.on_top {
            1
        } else {
            0
        }
    }

    /// [W8] Puts `window` last in the raise order; the next restack shows it.
    fn lift(&mut self, window: Window) {
        self.stack.retain(|&w| w != window);
        self.stack.push(window);
    }

    /// [Y1] Places every window where its layout and state say, then restacks.
    pub(super) fn arrange(&mut self) -> Result<(), Error> {
        let tiled = self.tiled_windows();
        let desk = self.desk();
        let cells = match desk.layout {
            Layout::Tile if !tiled.is_empty() => {
                // [Y3] Windows come and go from the stack: keep its weights balanced.
                let mut shares: Vec<f32> =
                    tiled[1..].iter().map(|w| self.clients[w].share).collect();
                layout::balance(&mut shares);
                for (window, &share) in tiled[1..].iter().zip(&shares) {
                    self.clients.get_mut(window).expect("tiled windows are managed").share = share;
                }
                layout::tile(self.screen, desk.master, &shares)
            }
            Layout::Max => vec![self.screen; tiled.len()],
            _ => Vec::new(),
        };
        let mut moved = false;
        for (&window, &cell) in tiled.iter().zip(&cells) {
            moved |= self.place(window, cell, self.border.width)?;
        }
        let others: Vec<Window> =
            self.order.iter().copied().filter(|&w| self.visible(w) && !self.tiled(w)).collect();
        for window in others {
            let c = &self.clients[&window];
            let (rect, border) = if c.fullscreen {
                (self.root_rect, 0)
            } else if c.maximized {
                (self.screen, self.border.width)
            } else {
                (c.rect, self.border.width)
            };
            moved |= self.place(window, rect, border)?;
        }
        // [F6] A window put on top, or taken off, shows it.
        let windows: Vec<Window> = self.clients.keys().copied().collect();
        for window in windows {
            self.paint_border(window, self.focused == Some(window))?;
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
        stack.retain(|&w| self.visible(w));
        stack.sort_by_key(|&w| self.layer(w));
        // [E4] Docks go above every window but a fullscreen one.
        let fullscreen = stack.iter().position(|&w| self.clients[&w].fullscreen);
        let mut docks: Vec<Window> = self.docks.keys().copied().collect();
        docks.sort_unstable();
        let at = fullscreen.unwrap_or(stack.len());
        stack.splice(at..at, docks);
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
    pub(super) fn ignore_crossings(&mut self) -> Result<(), Error> {
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
        let index = self.layouts.iter().position(|&l| l == self.desk().layout).unwrap_or(0);
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
        self.desk_mut().layout = layout;
        self.arrange()
    }

    /// [Y4]
    pub(super) fn master_width(&mut self, delta: f32) -> Result<(), Error> {
        let master = &mut self.desk_mut().master;
        master.width = (master.width + delta).clamp(MIN_FACTOR, MAX_FACTOR);
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

    /// Takes a tiled window out of the layout where it is ([W1], [W7]).
    /// A tiled, maximized or fullscreen window floats where it shows ([W7], [G1], [G2]).
    pub(super) fn float(&mut self, window: Window) {
        let tiled = self.tiled(window);
        let rect = self.placed(window);
        let client = self.clients.get_mut(&window).expect("managed");
        if tiled || client.maximized || client.fullscreen {
            (client.maximized, client.fullscreen) = (false, false);
            client.floating = true;
            client.rect = rect;
            // [W1] It no longer has a layer of its own to rise into.
            self.lift(window);
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
        self.set_flag(window, state, None);
        self.arrange()
    }

    /// [W3] [W4] [W5] [E7] Sets a state, or toggles it with `None`; the caller arranges.
    /// Floating goes through `float`, which keeps where the window shows.
    pub(super) fn set_flag(&mut self, window: Window, state: State, value: Option<bool>) {
        let client = self.clients.get_mut(&window).expect("managed");
        let flag = match state {
            State::Maximized => &mut client.maximized,
            State::Fullscreen => &mut client.fullscreen,
            State::OnTop => &mut client.on_top,
            State::Floating => unreachable!("floating goes through float()"),
        };
        *flag = value.unwrap_or(!*flag);
        // [W3] A window maximized now is raised; one on top or fullscreen has its layer.
        if state == State::Maximized && client.maximized {
            self.lift(window);
        }
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
        self.clients.get_mut(&window).expect("managed").minimized = true;
        self.minimized.push(window);
        self.show(window)?;
        self.arrange()?;
        self.refocus(window)
    }

    /// [W6] [V2] The most recently minimized window of the current workspace.
    pub(super) fn restore(&mut self) -> Result<(), Error> {
        let current = self.current;
        let last = self.minimized.iter().rev().find(|w| self.clients[w].workspace == current);
        match last.copied() {
            Some(window) => self.restore_window(window),
            None => Ok(()),
        }
    }

    /// [W6] Shows and focuses `window` if it is on the current workspace.
    pub(super) fn restore_window(&mut self, window: Window) -> Result<(), Error> {
        self.minimized.retain(|&w| w != window);
        self.clients.get_mut(&window).expect("minimized windows are managed").minimized = false;
        self.arrange()?;
        self.show(window)?;
        // `focus` refuses a window on another workspace.
        self.focus(window, true)
    }

    /// After a failed restart: gives the windows sugoi's borders back, and hides again
    /// what `release_all` mapped.
    pub(super) fn reclaim(&mut self) -> Result<(), Error> {
        for client in self.clients.values_mut() {
            client.placed = None;
        }
        for window in self.order.clone() {
            self.show(window)?;
        }
        self.regrab(true)?;
        self.arrange()
    }

    /// [G1] [G2] [G3] Starts dragging the window the last button press hit.
    /// The press is the one being handled: a `move` from a key binding finds none.
    pub(super) fn start_drag(&mut self, resize: bool) -> Result<(), Error> {
        let Some((x, y, Some(window))) = self.press.take() else { return Ok(()) };
        if !self.visible(window) {
            return Ok(());
        }
        let tiled = self.tiled(window);
        let layout = self.desk().layout;
        if tiled && layout != Layout::Tile {
            return Ok(());
        }
        let shown = self.placed(window);
        let kind = match (tiled, resize) {
            (true, true) => Kind::Layout,
            (true, false) => Kind::Swap,
            (false, false) => Kind::Move,
            (false, true) => Kind::Resize {
                left: i32::from(x) < shown.x + shown.width as i32 / 2,
                top: i32::from(y) < shown.y + shown.height as i32 / 2,
            },
        };
        let cursor = match kind {
            Kind::Move | Kind::Swap => self.cursors.moving,
            Kind::Resize { left, top } => {
                self.cursors.corners[2 * usize::from(!top) + usize::from(!left)]
            }
            Kind::Layout => self.cursors.layout,
        };
        // Grab first: if another client holds the pointer, nothing changes.
        let mask = EventMask::POINTER_MOTION | EventMask::BUTTON_RELEASE;
        let (root, async_) = (self.root, GrabMode::ASYNC);
        let grab =
            self.conn.grab_pointer(false, root, mask, async_, async_, NONE, cursor, CURRENT_TIME);
        if grab?.reply().map_err(lost)?.status != GrabStatus::SUCCESS {
            return Ok(());
        }
        let mut pointer = (x, y);
        if !tiled {
            self.float(window);
            self.arrange()?;
            self.raise(window)?;
        }
        let rect = self.clients[&window].rect;
        let target = match kind {
            Kind::Resize { left, top } => {
                let right = rect.x + rect.width as i32 - 1;
                let bottom = rect.y + rect.height as i32 - 1;
                Some((if left { rect.x } else { right }, if top { rect.y } else { bottom }))
            }
            Kind::Layout => Some(self.layout_handle(window)),
            Kind::Move | Kind::Swap => None,
        };
        if let Some((tx, ty)) = target {
            // Kept on screen, where X would clamp the warp anyway.
            let screen = self.root_rect;
            let tx = tx.clamp(screen.x, screen.x + screen.width as i32 - 1) as i16;
            let ty = ty.clamp(screen.y, screen.y + screen.height as i32 - 1) as i16;
            self.conn.warp_pointer(NONE, self.root, 0, 0, 0, 0, tx, ty)?;
            pointer = (tx, ty);
        }
        self.drag = Some(Drag { window, kind, pointer, rect });
        Ok(())
    }

    /// [G3] Where a layout drag starts: on the edge between the columns, at the window's
    /// bottom edge, its top one if it is the last of its column, its middle if alone.
    fn layout_handle(&self, window: Window) -> (i32, i32) {
        let tiled = self.tiled_windows();
        let index = tiled.iter().position(|&w| w == window).expect("a tiled window");
        let master = self.desk().master;
        let shares: Vec<f32> = tiled[1..].iter().map(|w| self.clients[w].share).collect();
        let cell = layout::tile(self.screen, master, &shares)[index];
        let x = layout::column_edge(self.screen, master);
        let y = if index == 0 || tiled.len() == 2 {
            cell.y + cell.height as i32 / 2
        } else if index + 1 == tiled.len() {
            cell.y
        } else {
            cell.y + cell.height as i32
        };
        (x, y)
    }

    /// [G1] [G2] [G3]
    pub(super) fn drag_to(&mut self, x: i16, y: i16) -> Result<(), Error> {
        let Some(drag) = &self.drag else { return Ok(()) };
        let (window, kind) = (drag.window, drag.kind);
        // A key may have changed the window's state during the drag: it wins.
        // A switch or a move may have hidden it ([V2]).
        let tiled_drag = matches!(kind, Kind::Swap | Kind::Layout);
        let still = self.visible(window)
            && if tiled_drag {
                self.tiled(window) && self.desk().layout == Layout::Tile
            } else {
                let c = &self.clients[&window];
                !self.tiled(window) && !(c.maximized || c.fullscreen)
            };
        if !still {
            return self.end_drag();
        }
        match kind {
            Kind::Swap => self.swap_under(window, x, y),
            Kind::Layout => self.resize_layout(window, x, y),
            Kind::Move | Kind::Resize { .. } => {
                let rect = self.dragged_rect(x, y);
                self.clients.get_mut(&window).expect("managed").rect = rect;
                // Only the dragged window moves: no arrange, no round trip per motion.
                self.place(window, rect, self.border.width)?;
                Ok(())
            }
        }
    }

    /// [G1] [G2] The floating window's rectangle with the pointer at (x, y).
    fn dragged_rect(&self, x: i16, y: i16) -> Rect {
        let drag = self.drag.as_ref().expect("dragging");
        let (dx, dy) = (i32::from(x - drag.pointer.0), i32::from(y - drag.pointer.1));
        let mut rect = drag.rect;
        let least = 2 * i32::from(self.border.width) + 1;
        match drag.kind {
            Kind::Resize { left, top } => {
                let (width, height) = (rect.width as i32, rect.height as i32);
                let new_width = (if left { width - dx } else { width + dx }).max(least);
                let new_height = (if top { height - dy } else { height + dy }).max(least);
                if left {
                    rect.x += width - new_width;
                }
                if top {
                    rect.y += height - new_height;
                }
                (rect.width, rect.height) = (new_width as u32, new_height as u32);
            }
            _ => {
                rect.x += dx;
                rect.y += dy;
            }
        }
        rect
    }

    /// [G1] Trades places with the tiled window under the pointer, if another.
    fn swap_under(&mut self, window: Window, x: i16, y: i16) -> Result<(), Error> {
        let (x, y) = (i32::from(x), i32::from(y));
        let under = self.tiled_windows().into_iter().find(|&w| {
            let r = self.placed(w);
            w != window
                && (r.x..r.x + r.width as i32).contains(&x)
                && (r.y..r.y + r.height as i32).contains(&y)
        });
        let Some(other) = under else { return Ok(()) };
        self.swap_windows(window, other)
    }

    /// [D5] [G1] Two windows trade places in the window order; two in the layout trade
    /// their shares too, so the cells stay where they were.
    pub(super) fn swap_windows(&mut self, a: Window, b: Window) -> Result<(), Error> {
        let i = self.order.iter().position(|&w| w == a);
        let j = self.order.iter().position(|&w| w == b);
        let (Some(i), Some(j)) = (i, j) else { return Ok(()) };
        self.order.swap(i, j);
        // In the layout by their own state, whatever the workspace's layout.
        let laid_out = |w: &Window| {
            let c = &self.clients[w];
            !(c.floating || c.maximized || c.fullscreen || c.minimized)
        };
        if laid_out(&a) && laid_out(&b) {
            let share = self.clients[&a].share;
            let client = self.clients.get_mut(&b).expect("managed");
            let other = std::mem::replace(&mut client.share, share);
            self.clients.get_mut(&a).expect("managed").share = other;
        }
        self.arrange()
    }

    /// [G3] The master width from x, the window's edge in its column from y.
    fn resize_layout(&mut self, window: Window, x: i16, y: i16) -> Result<(), Error> {
        let area = self.screen;
        let master = &mut self.desk_mut().master;
        master.width = layout::master_width_at(area, master.side, i32::from(x));
        let tiled = self.tiled_windows();
        let index = tiled.iter().position(|&w| w == window).expect("a tiled window");
        if index > 0 && tiled.len() > 2 {
            let mut shares: Vec<f32> = tiled[1..].iter().map(|w| self.clients[w].share).collect();
            let edge = (i32::from(y) - area.y) as f32 / area.height as f32;
            layout::share_to_edge(&mut shares, index - 1, edge.clamp(0.0, 1.0));
            for (w, share) in tiled[1..].iter().zip(shares) {
                self.clients.get_mut(w).expect("tiled windows are managed").share = share;
            }
        }
        self.arrange()
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
        if self.clients.contains_key(&gone) {
            self.unfocus(gone)?;
        }
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
        self.visible(window) && self.clients[&window].hints.focusable()
    }

    /// Gives `window` the border color of an unfocused window.
    pub(super) fn unfocus(&mut self, window: Window) -> Result<(), Error> {
        self.paint_border(window, false)
    }

    /// [F6] Paints `window`'s border for its focus and whether it is on top, when that
    /// color is not the one it has.
    pub(super) fn paint_border(&mut self, window: Window, focused: bool) -> Result<(), Error> {
        let client = self.clients.get_mut(&window).expect("painted windows are managed");
        let color = self.border.color(focused, client.on_top);
        if client.border_color != Some(color) {
            client.border_color = Some(color);
            let aux = ChangeWindowAttributesAux::new().border_pixel(color);
            self.conn.change_window_attributes(window, &aux)?;
        }
        Ok(())
    }
}
