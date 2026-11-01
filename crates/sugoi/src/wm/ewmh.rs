//! EWMH: what sugoi publishes and the requests it takes (`doc/spec/ewmh.md`).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, NONE};

use super::{Wm, vanished};
use crate::layout::Rect;
use crate::workspace::COUNT;
use crate::{Error, Goto, State};

/// [E7] `_NET_WM_STATE` actions.
const REMOVE: u32 = 0;
const ADD: u32 = 1;
const TOGGLE: u32 = 2;

/// What sugoi last wrote on the root, so `publish` writes only changes.
#[derive(Default)]
pub(super) struct Published {
    clients: Vec<Window>,
    stacking: Vec<Window>,
    active: Option<Option<Window>>,
    workarea: Option<Rect>,
}

impl Wm {
    /// [E1] [E3] The check window and the properties that never change.
    pub(super) fn setup_ewmh(&mut self) -> Result<(), Error> {
        let a = &self.atoms;
        let check = self.conn.generate_id()?;
        let aux = CreateWindowAux::new().override_redirect(1);
        let class = WindowClass::INPUT_ONLY;
        self.conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            check,
            self.root,
            -1,
            -1,
            1,
            1,
            0,
            class,
            COPY_FROM_PARENT,
            &aux,
        )?;
        for window in [self.root, check] {
            let checks = [check];
            let (property, kind) = (a._NET_SUPPORTING_WM_CHECK, AtomEnum::WINDOW);
            self.conn.change_property32(PropMode::REPLACE, window, property, kind, &checks)?;
        }
        let utf8 = a.UTF8_STRING;
        self.conn.change_property8(PropMode::REPLACE, check, a._NET_WM_NAME, utf8, b"sugoi")?;
        let supported = [
            a._NET_SUPPORTED,
            a._NET_SUPPORTING_WM_CHECK,
            a._NET_CLIENT_LIST,
            a._NET_CLIENT_LIST_STACKING,
            a._NET_ACTIVE_WINDOW,
            a._NET_NUMBER_OF_DESKTOPS,
            a._NET_DESKTOP_NAMES,
            a._NET_CURRENT_DESKTOP,
            a._NET_DESKTOP_GEOMETRY,
            a._NET_DESKTOP_VIEWPORT,
            a._NET_WORKAREA,
            a._NET_WM_DESKTOP,
            a._NET_WM_NAME,
            a._NET_WM_WINDOW_TYPE,
            a._NET_WM_WINDOW_TYPE_DOCK,
            a._NET_WM_STRUT,
            a._NET_WM_STRUT_PARTIAL,
            a._NET_WM_STATE,
            a._NET_WM_STATE_FULLSCREEN,
            a._NET_WM_STATE_ABOVE,
            a._NET_WM_STATE_HIDDEN,
            a._NET_WM_STATE_MAXIMIZED_VERT,
            a._NET_WM_STATE_MAXIMIZED_HORZ,
            a._NET_WM_ALLOWED_ACTIONS,
            a._NET_CLOSE_WINDOW,
        ];
        let (root, atom) = (self.root, AtomEnum::ATOM);
        self.conn.change_property32(PropMode::REPLACE, root, a._NET_SUPPORTED, atom, &supported)?;
        let cardinal = AtomEnum::CARDINAL;
        let count = [COUNT as u32];
        self.conn.change_property32(
            PropMode::REPLACE,
            root,
            a._NET_NUMBER_OF_DESKTOPS,
            cardinal,
            &count,
        )?;
        let names: Vec<u8> = (1..=COUNT).flat_map(|n| format!("{n}\0").into_bytes()).collect();
        self.conn.change_property8(PropMode::REPLACE, root, a._NET_DESKTOP_NAMES, utf8, &names)?;
        let size = [self.root_rect.width, self.root_rect.height];
        self.conn.change_property32(
            PropMode::REPLACE,
            root,
            a._NET_DESKTOP_GEOMETRY,
            cardinal,
            &size,
        )?;
        let viewports = vec![0u32; 2 * COUNT];
        self.conn.change_property32(
            PropMode::REPLACE,
            root,
            a._NET_DESKTOP_VIEWPORT,
            cardinal,
            &viewports,
        )?;
        Ok(())
    }

    /// [E6] Set once, when a window is managed.
    pub(super) fn set_allowed_actions(&self, window: Window) -> Result<(), Error> {
        let a = &self.atoms;
        let actions = [
            a._NET_WM_ACTION_MOVE,
            a._NET_WM_ACTION_RESIZE,
            a._NET_WM_ACTION_FULLSCREEN,
            a._NET_WM_ACTION_MAXIMIZE_HORZ,
            a._NET_WM_ACTION_MAXIMIZE_VERT,
            a._NET_WM_ACTION_MINIMIZE,
            a._NET_WM_ACTION_CLOSE,
            a._NET_WM_ACTION_CHANGE_DESKTOP,
        ];
        let (property, kind) = (a._NET_WM_ALLOWED_ACTIONS, AtomEnum::ATOM);
        self.conn.change_property32(PropMode::REPLACE, window, property, kind, &actions)?;
        Ok(())
    }

    /// [E2] [E4] [E5] Writes what changed since the last call; the event loop calls it
    /// before it waits, so every path that changes state is covered.
    pub(super) fn publish(&mut self) -> Result<(), Error> {
        let (root, window_kind) = (self.root, AtomEnum::WINDOW);
        if self.managed != self.published.clients {
            let clients = self.managed.clone();
            let property = self.atoms._NET_CLIENT_LIST;
            self.conn.change_property32(
                PropMode::REPLACE,
                root,
                property,
                window_kind,
                &clients,
            )?;
            self.published.clients = clients;
        }
        let mut stacking = self.stack.clone();
        stacking.sort_by_key(|&w| self.layer(w));
        if stacking != self.published.stacking {
            let property = self.atoms._NET_CLIENT_LIST_STACKING;
            self.conn.change_property32(
                PropMode::REPLACE,
                root,
                property,
                window_kind,
                &stacking,
            )?;
            self.published.stacking = stacking;
        }
        if self.published.active != Some(self.focused) {
            let active = [self.focused.unwrap_or(NONE)];
            let property = self.atoms._NET_ACTIVE_WINDOW;
            self.conn.change_property32(PropMode::REPLACE, root, property, window_kind, &active)?;
            self.published.active = Some(self.focused);
        }
        if self.published.workarea != Some(self.screen) {
            let s = self.screen;
            let one = [s.x as u32, s.y as u32, s.width, s.height];
            let all: Vec<u32> = one.iter().copied().cycle().take(4 * COUNT).collect();
            let (property, cardinal) = (self.atoms._NET_WORKAREA, AtomEnum::CARDINAL);
            self.conn.change_property32(PropMode::REPLACE, root, property, cardinal, &all)?;
            self.published.workarea = Some(s);
        }
        let windows: Vec<Window> = self.clients.keys().copied().collect();
        for window in windows {
            let state = self.wm_state_atoms(window);
            let client = self.clients.get_mut(&window).expect("managed");
            if client.net_state.as_ref() != Some(&state) {
                client.net_state = Some(state.clone());
                let (property, atom) = (self.atoms._NET_WM_STATE, AtomEnum::ATOM);
                self.conn.change_property32(PropMode::REPLACE, window, property, atom, &state)?;
            }
        }
        Ok(())
    }

    /// [E5]
    fn wm_state_atoms(&self, window: Window) -> Vec<Atom> {
        let (a, c) = (&self.atoms, &self.clients[&window]);
        let mut state = Vec::new();
        if c.fullscreen {
            state.push(a._NET_WM_STATE_FULLSCREEN);
        }
        if c.on_top {
            state.push(a._NET_WM_STATE_ABOVE);
        }
        if !self.visible(window) {
            state.push(a._NET_WM_STATE_HIDDEN);
        }
        if c.maximized {
            state.extend([a._NET_WM_STATE_MAXIMIZED_VERT, a._NET_WM_STATE_MAXIMIZED_HORZ]);
        }
        state
    }

    /// [E7] to [E10]
    pub(super) fn client_message(&mut self, e: &ClientMessageEvent) -> Result<(), Error> {
        let a = &self.atoms;
        let data = e.data.as_data32();
        let managed = self.clients.contains_key(&e.window);
        if e.type_ == a._NET_WM_STATE && managed {
            self.state_request(e.window, data[0], [data[1], data[2]])
        } else if e.type_ == a._NET_ACTIVE_WINDOW && managed {
            self.activation_request(e.window, data[0])
        } else if e.type_ == a._NET_CURRENT_DESKTOP {
            self.switch(Goto::Index(data[0] as usize))
        } else if e.type_ == a._NET_WM_DESKTOP && managed {
            self.move_to_index(e.window, data[0] as usize)
        } else if e.type_ == a._NET_WM_DESKTOP
            && e.window != self.root
            && !self.docks.contains_key(&e.window)
        {
            // [V10] Before the window maps, the property says where it goes, as if its
            // app had set it.
            match data[0] as usize {
                index if index < COUNT => self.set_desktop(e.window, index),
                _ => Ok(()),
            }
        } else if e.type_ == a._NET_CLOSE_WINDOW && managed {
            self.close(e.window)
        } else {
            Ok(())
        }
    }

    /// [E7]
    fn state_request(
        &mut self,
        window: Window,
        action: u32,
        atoms: [Atom; 2],
    ) -> Result<(), Error> {
        let mut states: Vec<State> = Vec::new();
        for state in atoms.into_iter().filter_map(|atom| self.net_state(atom)) {
            if !states.contains(&state) {
                states.push(state);
            }
        }
        let value = match action {
            REMOVE => Some(false),
            ADD => Some(true),
            TOGGLE => None,
            _ => return Ok(()),
        };
        for state in states {
            self.set_flag(window, state, value);
        }
        self.arrange()
    }

    /// [E5] [E7] The state an EWMH state atom names, of those sugoi has.
    pub(super) fn net_state(&self, atom: Atom) -> Option<State> {
        let a = &self.atoms;
        Some(match atom {
            t if t == a._NET_WM_STATE_FULLSCREEN => State::Fullscreen,
            t if t == a._NET_WM_STATE_ABOVE => State::OnTop,
            t if t == a._NET_WM_STATE_MAXIMIZED_VERT || t == a._NET_WM_STATE_MAXIMIZED_HORZ => {
                State::Maximized
            }
            _ => return None,
        })
    }

    /// [E8] A pager or the user (source 2) gets the window wherever it is; an app (1, or
    /// 0 from old clients) only one on the current workspace, restored if minimized, so
    /// it cannot pull the user to another workspace.
    fn activation_request(&mut self, window: Window, source: u32) -> Result<(), Error> {
        const PAGER: u32 = 2;
        let here = self.clients[&window].workspace == self.current;
        if source == PAGER || here { self.activate(window) } else { Ok(()) }
    }

    /// [E8] Shows `window` wherever it is, and focuses it.
    pub(super) fn activate(&mut self, window: Window) -> Result<(), Error> {
        let workspace = self.clients[&window].workspace;
        if workspace != self.current {
            self.switch(Goto::Index(workspace))?;
        }
        if self.clients.get(&window).is_some_and(|c| c.minimized) {
            return self.restore_window(window);
        }
        self.focus(window, true)
    }

    /// [E4] A dock is mapped, not managed; its struts make the work area.
    pub(super) fn add_dock(&mut self, window: Window, map: bool) -> Result<(), Error> {
        let strut = self.read_strut(window)?;
        self.docks.insert(window, strut);
        if map {
            self.conn.map_window(window)?;
        }
        self.update_workarea()?;
        // X maps it on top: below a fullscreen window it goes, whatever its strut.
        if self.restack()? {
            self.ignore_crossings()?;
        }
        Ok(())
    }

    pub(super) fn remove_dock(&mut self, window: Window) -> Result<(), Error> {
        self.docks.remove(&window);
        self.update_workarea()
    }

    pub(super) fn update_strut(&mut self, window: Window) -> Result<(), Error> {
        let strut = self.read_strut(window)?;
        self.docks.insert(window, strut);
        self.update_workarea()
    }

    /// [E4] Left, right, top, bottom: `_NET_WM_STRUT_PARTIAL`'s first four, else
    /// `_NET_WM_STRUT`'s.
    fn read_strut(&self, window: Window) -> Result<[u32; 4], Error> {
        let a = &self.atoms;
        let cardinal = AtomEnum::CARDINAL;
        let partial =
            self.conn.get_property(false, window, a._NET_WM_STRUT_PARTIAL, cardinal, 0, 12)?;
        let plain = self.conn.get_property(false, window, a._NET_WM_STRUT, cardinal, 0, 4)?;
        let read = |reply: Option<GetPropertyReply>| -> Option<[u32; 4]> {
            let values: Vec<u32> = reply?.value32()?.take(4).collect();
            values.try_into().ok()
        };
        let partial = read(vanished(partial.reply())?);
        let plain = read(vanished(plain.reply())?);
        Ok(partial.or(plain).unwrap_or_default())
    }

    /// [E4] [Y1] The screen less every dock's struts; arranges again when it changes.
    fn update_workarea(&mut self) -> Result<(), Error> {
        let full = self.root_rect;
        let [mut left, mut right, mut top, mut bottom] = [0u32; 4];
        for strut in self.docks.values() {
            left = left.max(strut[0]);
            right = right.max(strut[1]);
            top = top.max(strut[2]);
            bottom = bottom.max(strut[3]);
        }
        // A strut past the screen, from a bad dock, leaves at least a pixel.
        let (left, top) = (left.min(full.width - 1), top.min(full.height - 1));
        let width = (full.width - left).saturating_sub(right).max(1);
        let height = (full.height - top).saturating_sub(bottom).max(1);
        let area = Rect::new(full.x + left as i32, full.y + top as i32, width, height);
        if area != self.screen {
            self.screen = area;
            self.arrange()?;
        }
        Ok(())
    }
}
