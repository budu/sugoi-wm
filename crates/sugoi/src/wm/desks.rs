//! Workspaces: which windows show, switching and moving (`doc/spec/workspaces.md`).

use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use super::{NORMAL_STATE, POINTER_ROOT, Wm};
use crate::layout::{Layout, Master};
use crate::workspace::{self, COUNT};
use crate::{Error, Goto};

/// `IconicState` in `WM_STATE` (ICCCM 4.1.3.1).
pub(super) const ICONIC_STATE: u32 = 3;

/// [V3] What each workspace has of its own.
#[derive(Clone, Copy)]
pub(super) struct Desk {
    pub(super) layout: Layout,
    pub(super) master: Master,
}

/// [V9] The workspace index in a `_NET_WM_DESKTOP`-like reply, if valid; `None` for a
/// window gone too ([M5]).
pub(super) fn desktop(reply: Result<GetPropertyReply, ReplyError>) -> Result<Option<usize>, Error> {
    let Some(reply) = super::vanished(reply)? else { return Ok(None) };
    let value = reply.value32().and_then(|mut v| v.next());
    Ok(value.map(|v| v as usize).filter(|&v| v < COUNT))
}

impl Wm {
    /// [V2] Whether `window` is on the current workspace and not minimized.
    pub(super) fn visible(&self, window: Window) -> bool {
        self.clients.get(&window).is_some_and(|c| c.workspace == self.current && !c.minimized)
    }

    /// [V2] Maps or unmaps `window` to match whether it shows; an unmap's sequence number
    /// is kept, so its `UnmapNotify` is not taken for a withdrawal ([W6]).
    pub(super) fn show(&mut self, window: Window) -> Result<(), Error> {
        let visible = self.visible(window);
        let client = self.clients.get_mut(&window).expect("shown windows are managed");
        if client.mapped == visible {
            return Ok(());
        }
        client.mapped = visible;
        if visible {
            self.conn.map_window(window)?;
            self.set_state(window, NORMAL_STATE)
        } else {
            let sequence = self.conn.unmap_window(window)?.sequence_number();
            client.unmaps.push_back(sequence as u16);
            // [G1] A hidden window cannot be dragged on.
            if self.drag.as_ref().is_some_and(|d| d.window() == window) {
                self.end_drag()?;
            }
            self.set_state(window, ICONIC_STATE)
        }
    }

    /// [V9]
    pub(super) fn set_current_desktop(&self) -> Result<(), Error> {
        let (atom, cardinal) = (self.atoms._NET_CURRENT_DESKTOP, AtomEnum::CARDINAL);
        let value = [self.current as u32];
        self.conn.change_property32(PropMode::REPLACE, self.root, atom, cardinal, &value)?;
        Ok(())
    }

    pub(super) fn set_state(&self, window: Window, state: u32) -> Result<(), Error> {
        let atom = self.atoms.WM_STATE;
        self.conn.change_property32(PropMode::REPLACE, window, atom, atom, &[state, NONE])?;
        Ok(())
    }

    /// [V9]
    pub(super) fn set_desktop(&self, window: Window, workspace: usize) -> Result<(), Error> {
        let (atom, cardinal) = (self.atoms._NET_WM_DESKTOP, AtomEnum::CARDINAL);
        let value = [workspace as u32];
        self.conn.change_property32(PropMode::REPLACE, window, atom, cardinal, &value)?;
        Ok(())
    }

    /// [V9] A workspace index stored in `property` on `window`, if valid.
    pub(super) fn read_desktop(
        &self,
        window: Window,
        property: Atom,
    ) -> Result<Option<usize>, Error> {
        let cookie = self.conn.get_property(false, window, property, AtomEnum::CARDINAL, 0, 1)?;
        desktop(cookie.reply())
    }

    /// [V4] to [V7]
    pub(super) fn switch(&mut self, goto: Goto) -> Result<(), Error> {
        let target = match goto {
            Goto::Number(n) => match workspace::target(self.current, n) {
                Some(target) => target,
                None => return Ok(()),
            },
            Goto::Next => workspace::step(self.current, true),
            Goto::Previous => workspace::step(self.current, false),
            Goto::Back => self.back,
        };
        if target == self.current {
            return Ok(());
        }
        self.back = self.current;
        self.current = target;
        self.set_current_desktop()?;
        // Place the new workspace's windows, show them, then hide the old ones.
        self.arrange()?;
        let (show, hide): (Vec<Window>, Vec<Window>) =
            self.order.iter().copied().partition(|&w| self.visible(w));
        for window in show.into_iter().chain(hide) {
            self.show(window)?;
        }
        self.ignore_crossings()?;
        // [V7]
        if let Some(previous) = self.focused.take() {
            self.unfocus(previous)?;
        }
        match self.history.iter().copied().find(|&w| self.can_focus(w)) {
            Some(window) => self.focus(window, false),
            None => {
                self.conn.set_input_focus(InputFocus::POINTER_ROOT, POINTER_ROOT, CURRENT_TIME)?;
                Ok(())
            }
        }
    }

    /// [V8]
    pub(super) fn move_to(&mut self, n: u32) -> Result<(), Error> {
        let Some(window) = self.focused else { return Ok(()) };
        let from = self.clients[&window].workspace;
        let Some(target) = workspace::target(from, n) else { return Ok(()) };
        if target == from {
            return Ok(());
        }
        self.clients.get_mut(&window).expect("managed").workspace = target;
        self.set_desktop(window, target)?;
        self.show(window)?;
        self.arrange()?;
        self.refocus(window)
    }

    /// [V3] The current workspace's layout and master column.
    pub(super) fn desk(&self) -> Desk {
        self.desks[self.current]
    }

    pub(super) fn desk_mut(&mut self) -> &mut Desk {
        &mut self.desks[self.current]
    }
}
