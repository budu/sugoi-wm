//! The keyboard mapping: keysyms to keycodes ([R3]) and the Num Lock modifier ([R4]).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, Keycode, ModMask};
use x11rb::rust_connection::RustConnection;

use crate::Error;

/// `XK_Num_Lock`.
const NUM_LOCK: u32 = 0xff7f;

pub struct Keyboard {
    min_keycode: u8,
    per_keycode: usize,
    keysyms: Vec<u32>,
    pub num_lock: ModMask,
}

impl Keyboard {
    pub fn read(conn: &RustConnection) -> Result<Keyboard, Error> {
        let setup = conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let mapping = conn.get_keyboard_mapping(min, max - min + 1)?;
        let modifiers = conn.get_modifier_mapping()?;
        let mapping = mapping.reply().map_err(crate::wm::lost)?;
        let modifiers = modifiers.reply().map_err(crate::wm::lost)?;
        let mut keyboard = Keyboard {
            min_keycode: min,
            per_keycode: usize::from(mapping.keysyms_per_keycode),
            keysyms: mapping.keysyms,
            num_lock: ModMask::default(),
        };
        // The modifier map lists keycodes_per_modifier keycodes for each of 8 modifiers.
        let per_modifier = usize::from(modifiers.keycodes_per_modifier());
        let num_lock = keyboard.keycodes(NUM_LOCK);
        for (index, keycodes) in modifiers.keycodes.chunks(per_modifier.max(1)).enumerate() {
            if keycodes.iter().any(|k| num_lock.contains(k)) {
                keyboard.num_lock = ModMask::from(1u16 << index);
            }
        }
        Ok(keyboard)
    }

    /// The keycodes that produce `keysym` without a level shift or with Shift.
    pub fn keycodes(&self, keysym: u32) -> Vec<Keycode> {
        let mut found = Vec::new();
        for (index, syms) in self.keysyms.chunks(self.per_keycode.max(1)).enumerate() {
            if syms.iter().take(2).any(|&s| s == keysym) {
                found.push(self.min_keycode + index as u8);
            }
        }
        found
    }
}
