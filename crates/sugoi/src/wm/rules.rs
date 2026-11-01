//! Rules and hooks (`doc/spec/rules.md`).

use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::properties::WmClass;
use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ConnectionExt as _, QueryClientIdsReply};
use x11rb::protocol::xproto::*;

use super::{Wm, vanished};
use crate::layout;
use crate::{Effects, Error, Event, HookArg, Info, Snapshot, WindowCommand, WindowType};

/// The property cookies of [U1], sent before any reply is read.
pub(super) struct InfoCookies<'c> {
    class: x11rb::properties::WmClassCookie<'c, x11rb::rust_connection::RustConnection>,
    net_name: x11rb::cookie::Cookie<'c, x11rb::rust_connection::RustConnection, GetPropertyReply>,
    name: x11rb::cookie::Cookie<'c, x11rb::rust_connection::RustConnection, GetPropertyReply>,
    kind: x11rb::cookie::Cookie<'c, x11rb::rust_connection::RustConnection, GetPropertyReply>,
    transient: x11rb::cookie::Cookie<'c, x11rb::rust_connection::RustConnection, GetPropertyReply>,
    /// None when the process is known already, or the server has no X-Resource.
    pid: Option<
        x11rb::cookie::Cookie<'c, x11rb::rust_connection::RustConnection, QueryClientIdsReply>,
    >,
}

impl Wm {
    /// [U1] Asks for what a window is.
    /// [U1] Asks for what a window is; `pid` also for its process ([A2]), which a
    /// window's connection keeps for life, so only at manage.
    pub(super) fn request_info(&self, window: Window, pid: bool) -> Result<InfoCookies<'_>, Error> {
        let a = &self.atoms;
        let get = |property: Atom, kind: AtomEnum| {
            self.conn.get_property(false, window, property, kind, 0, 1024)
        };
        Ok(InfoCookies {
            class: WmClass::get(&self.conn, window)?,
            net_name: self.conn.get_property(
                false,
                window,
                a._NET_WM_NAME,
                a.UTF8_STRING,
                0,
                1024,
            )?,
            // Any type: STRING is Latin-1, but some clients write UTF8_STRING here.
            name: get(AtomEnum::WM_NAME.into(), AtomEnum::ANY)?,
            kind: get(a._NET_WM_WINDOW_TYPE, AtomEnum::ATOM)?,
            transient: get(AtomEnum::WM_TRANSIENT_FOR.into(), AtomEnum::WINDOW)?,
            // [A2] The server knows the process behind each local connection.
            pid: if pid { self.request_pid(window)? } else { None },
        })
    }

    /// [A2] The server knows the process behind each local connection; without the
    /// X-Resource extension, nothing is known.
    fn request_pid(
        &self,
        window: Window,
    ) -> Result<
        Option<
            x11rb::cookie::Cookie<'_, x11rb::rust_connection::RustConnection, QueryClientIdsReply>,
        >,
        Error,
    > {
        let spec = ClientIdSpec { client: window, mask: ClientIdMask::LOCAL_CLIENT_PID };
        match self.conn.res_query_client_ids(&[spec]) {
            Ok(cookie) => Ok(Some(cookie)),
            Err(ConnectionError::UnsupportedExtension) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// [U1] What a window is; missing or malformed properties read as empty.
    pub(super) fn read_info(&self, cookies: InfoCookies<'_>) -> Result<Info, Error> {
        let (class, instance) = match cookies.class.reply() {
            Ok(Some(c)) => (
                String::from_utf8_lossy(c.class()).into_owned(),
                String::from_utf8_lossy(c.instance()).into_owned(),
            ),
            _ => (String::new(), String::new()),
        };
        let title = self.title(cookies.net_name.reply(), cookies.name.reply())?;
        let types: Vec<Atom> = vanished(cookies.kind.reply())?
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default();
        let transient = vanished(cookies.transient.reply())?.is_some_and(|r| r.value_len > 0);
        // [A2] None for a remote client, or a server without X-Resource.
        let pid = match cookies.pid {
            Some(cookie) => vanished(cookie.reply())?.and_then(|r| {
                let id = r.ids.iter().find(|id| id.spec.mask == ClientIdMask::LOCAL_CLIENT_PID)?;
                id.value.first().copied()
            }),
            None => None,
        };
        Ok(Info {
            class,
            instance,
            title,
            kind: types.iter().find_map(|&t| self.window_type(t)),
            transient,
            pid,
        })
    }

    /// A text property: Latin-1 for `STRING`, UTF-8 for `UTF8_STRING`; other encodings,
    /// such as `COMPOUND_TEXT`, read as empty rather than garbled.
    fn decode(&self, reply: &GetPropertyReply) -> String {
        let string: Atom = AtomEnum::STRING.into();
        if reply.type_ == string {
            reply.value.iter().map(|&b| char::from(b)).collect()
        } else if reply.type_ == self.atoms.UTF8_STRING {
            String::from_utf8_lossy(&reply.value).into_owned()
        } else {
            String::new()
        }
    }

    /// [U1] `_NET_WM_NAME`, else `WM_NAME`.
    fn title(
        &self,
        net: Result<GetPropertyReply, ReplyError>,
        name: Result<GetPropertyReply, ReplyError>,
    ) -> Result<String, Error> {
        let net = vanished(net)?.map(|r| self.decode(&r)).unwrap_or_default();
        let name = vanished(name)?.map(|r| self.decode(&r)).unwrap_or_default();
        Ok(if net.is_empty() { name } else { net })
    }

    /// [U1] The title alone, for a title change.
    pub(super) fn read_title(&self, window: Window) -> Result<String, Error> {
        let a = &self.atoms;
        let net = self.conn.get_property(false, window, a._NET_WM_NAME, a.UTF8_STRING, 0, 1024)?;
        let any: Atom = AtomEnum::ANY.into();
        let name = self.conn.get_property(false, window, AtomEnum::WM_NAME, any, 0, 1024)?;
        self.title(net.reply(), name.reply())
    }

    fn window_type(&self, atom: Atom) -> Option<WindowType> {
        let a = &self.atoms;
        Some(match atom {
            t if t == a._NET_WM_WINDOW_TYPE_NORMAL => WindowType::Normal,
            t if t == a._NET_WM_WINDOW_TYPE_DIALOG => WindowType::Dialog,
            t if t == a._NET_WM_WINDOW_TYPE_SPLASH => WindowType::Splash,
            t if t == a._NET_WM_WINDOW_TYPE_UTILITY => WindowType::Utility,
            t if t == a._NET_WM_WINDOW_TYPE_TOOLBAR => WindowType::Toolbar,
            t if t == a._NET_WM_WINDOW_TYPE_MENU => WindowType::Menu,
            _ => return None,
        })
    }

    /// [U2] to [U5] Applies every matching rule to `window`; `adopted` for a window
    /// adopted at startup, which keeps its geometry, and `keep_workspace` for one on the
    /// workspace its `_NET_WM_DESKTOP` names ([U6]).
    pub(super) fn apply_rules(
        &mut self,
        window: Window,
        adopted: bool,
        keep_workspace: bool,
    ) -> Result<(), Error> {
        let info = &self.clients[&window].info;
        let effects = self
            .rules
            .iter()
            .filter(|rule| rule.matcher.matches(info))
            .fold(Effects::default(), |acc, rule| acc.then(&rule.effects));
        // [U6] An adopted window keeps the geometry it had: only new ones are placed.
        if !adopted {
            if let Some((width, height)) = effects.size {
                self.resize_rect(window, width, height);
            }
            if effects.center {
                self.center_rect(window);
            }
        }
        let client = self.clients.get_mut(&window).expect("ruled windows are managed");
        if let Some(float) = effects.float {
            client.floating = float;
        }
        if let Some(on_top) = effects.on_top {
            client.on_top = on_top;
        }
        if let Some(n) = effects.workspace.filter(|_| !keep_workspace) {
            client.workspace = n as usize - 1;
            self.set_desktop(window, n as usize - 1)?;
        }
        if effects.master {
            self.order.retain(|&w| w != window);
            self.order.insert(0, window);
        }
        Ok(())
    }

    /// [H2]
    pub(super) fn snapshot(&self, window: Window) -> Snapshot {
        let c = &self.clients[&window];
        Snapshot {
            id: window,
            info: c.info.clone(),
            unit: c.unit.clone(),
            workspace: c.workspace as u32 + 1,
            floating: c.floating || c.maximized || c.fullscreen || !self.tiled_layout(window),
        }
    }

    fn resize_rect(&mut self, window: Window, width: u32, height: u32) {
        let (screen, border) = (self.screen, 2 * u32::from(self.border.width));
        let client = self.clients.get_mut(&window).expect("managed");
        client.rect.width = width + border;
        client.rect.height = height + border;
        client.rect = layout::on_screen(screen, client.rect);
    }

    fn center_rect(&mut self, window: Window) {
        let screen = self.screen;
        let client = self.clients.get_mut(&window).expect("managed");
        client.rect = layout::center(screen, client.rect);
    }

    /// Whether `window`'s workspace tiles: in the float layout every window floats.
    fn tiled_layout(&self, window: Window) -> bool {
        self.desks[self.clients[&window].workspace].layout != layout::Layout::Float
    }

    /// [H1] [H4] Runs the hooks for `event`; nothing the hooks' actions cause runs any.
    /// Manage hooks run at once, before the window shows; the others once the event that
    /// raised them is handled, from the event loop, so no grab is held while Ruby runs and
    /// a switch or a close is complete when its hook sees it.
    pub(super) fn run_hooks(&mut self, event: Event, arg: HookArg) -> Result<(), Error> {
        if self.in_hook {
            return Ok(());
        }
        if event != Event::Manage {
            if self.hooks.iter().any(|(e, _)| *e == event) {
                self.later_hooks.push_back((event, arg));
            }
            return Ok(());
        }
        self.run_hooks_now(event, arg)
    }

    /// Runs the hooks deferred by `run_hooks`.
    pub(super) fn run_later_hooks(&mut self) -> Result<bool, Error> {
        let Some((event, arg)) = self.later_hooks.pop_front() else { return Ok(false) };
        self.run_hooks_now(event, arg)?;
        Ok(true)
    }

    fn run_hooks_now(&mut self, event: Event, arg: HookArg) -> Result<(), Error> {
        // [H2] A deferred hook sees its window as it is now, as `windows` does.
        let arg = match arg {
            HookArg::Window(s) if self.clients.contains_key(&s.id) => {
                HookArg::Window(self.snapshot(s.id))
            }
            other => other,
        };
        let blocks: Vec<usize> =
            self.hooks.iter().filter(|(e, _)| *e == event).map(|&(_, b)| b).collect();
        if blocks.is_empty() {
            return Ok(());
        }
        let windows: Vec<Snapshot> = self.order.iter().map(|&w| self.snapshot(w)).collect();
        self.in_hook = true;
        let loads = self.loads;
        let mut result = Ok(());
        for block in blocks {
            // A hook's `reload` replaced the blocks: the indices left are stale.
            if self.loads != loads {
                break;
            }
            match self.script.hook(block, &arg, &windows) {
                Ok(commands) => {
                    for command in commands {
                        result = self.execute(command);
                        if result.is_err() {
                            break;
                        }
                    }
                }
                Err(message) => super::report(&message),
            }
            if result.is_err() {
                break;
            }
        }
        self.in_hook = false;
        result
    }

    /// [H2]
    pub(super) fn window_command(
        &mut self,
        window: Window,
        command: WindowCommand,
    ) -> Result<(), Error> {
        if !self.clients.contains_key(&window) {
            return Ok(());
        }
        match command {
            WindowCommand::Float(true) => {
                self.float(window);
                self.clients.get_mut(&window).expect("managed").floating = true;
            }
            WindowCommand::Float(false) => {
                self.clients.get_mut(&window).expect("managed").floating = false;
            }
            // [W7] Like `center`, they float a tiled window.
            WindowCommand::Size(width, height) => {
                self.float(window);
                self.resize_rect(window, width, height);
            }
            WindowCommand::Center => {
                self.float(window);
                self.center_rect(window);
            }
            WindowCommand::Master => {
                self.order.retain(|&w| w != window);
                self.order.insert(0, window);
            }
            WindowCommand::MoveTo(n) => return self.move_window(window, n),
            WindowCommand::Close => return self.close(window),
            WindowCommand::Kill => return self.kill(window),
        }
        self.arrange()
    }
}
