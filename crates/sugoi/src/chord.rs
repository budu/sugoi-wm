//! Chord strings from the config ([D3], [D4]): `Mod-Shift-j`, `Mod-#10`, `Mod-1`.

use x11rb::protocol::xproto::ModMask;

/// The key part of a key chord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Sym(u32),
    Code(u8),
}

/// Parses a modifier name; `modkey` is what `Mod` stands for.
pub fn modifier(name: &str, modkey: ModMask) -> Option<ModMask> {
    Some(match name {
        "Mod" => modkey,
        "Shift" => ModMask::SHIFT,
        "Ctrl" | "Control" => ModMask::CONTROL,
        "Alt" | "Mod1" => ModMask::M1,
        "Mod2" => ModMask::M2,
        "Mod3" => ModMask::M3,
        "Super" | "Mod4" => ModMask::M4,
        "Mod5" => ModMask::M5,
        _ => return None,
    })
}

/// Parses the modkey itself, which cannot be `Mod`.
pub fn modkey(name: &str) -> Result<ModMask, String> {
    match name {
        "Mod" => None,
        _ => modifier(name, ModMask::default()),
    }
    .ok_or_else(|| format!("unknown modkey {name:?}"))
}

/// Splits `chord` into its modifiers and its last part.
fn split(chord: &str, modkey: ModMask) -> Result<(ModMask, &str), String> {
    let mut parts: Vec<&str> = chord.split('-').collect();
    let last = parts.pop().filter(|p| !p.is_empty());
    let Some(last) = last else {
        return Err(format!("chord {chord:?} has no key"));
    };
    let mut mods = ModMask::default();
    for part in parts {
        let Some(m) = modifier(part, modkey) else {
            return Err(format!("unknown modifier {part:?} in chord {chord:?}"));
        };
        mods |= m;
    }
    Ok((mods, last))
}

pub fn key(chord: &str, modkey: ModMask) -> Result<(ModMask, Key), String> {
    let (mods, last) = split(chord, modkey)?;
    if let Some(code) = last.strip_prefix('#') {
        let code = code.parse().map_err(|_| format!("bad keycode {last:?} in {chord:?}"))?;
        return Ok((mods, Key::Code(code)));
    }
    let sym = xkbcommon::xkb::keysym_from_name(last, xkbcommon::xkb::KEYSYM_NO_FLAGS).raw();
    if sym == 0 {
        return Err(format!("unknown key name {last:?} in chord {chord:?}"));
    }
    Ok((mods, Key::Sym(sym)))
}

pub fn button(chord: &str, modkey: ModMask) -> Result<(ModMask, u8), String> {
    let (mods, last) = split(chord, modkey)?;
    match last.parse() {
        Ok(button @ 1..=5) => Ok((mods, button)),
        _ => Err(format!("unknown button {last:?} in chord {chord:?}, expected 1 to 5")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOD4: ModMask = ModMask::M4;

    #[test]
    fn parses_keys_by_name_and_code() {
        assert_eq!(key("Mod-Shift-j", MOD4), Ok((ModMask::M4 | ModMask::SHIFT, Key::Sym(0x6a))));
        assert_eq!(key("Ctrl-Return", MOD4), Ok((ModMask::CONTROL, Key::Sym(0xff0d))));
        assert_eq!(key("Mod-#10", ModMask::M1), Ok((ModMask::M1, Key::Code(10))));
        assert_eq!(key("XF86AudioMute", MOD4), Ok((ModMask::default(), Key::Sym(0x1008ff12))));
    }

    #[test]
    fn rejects_bad_chords() {
        assert!(key("Mod-Nope", MOD4).unwrap_err().contains("unknown key name \"Nope\""));
        assert!(key("Hyper-j", MOD4).unwrap_err().contains("unknown modifier \"Hyper\""));
        assert!(key("Mod-", MOD4).unwrap_err().contains("has no key"));
        assert!(key("Mod-#x", MOD4).unwrap_err().contains("bad keycode"));
        assert!(button("Mod-6", MOD4).is_err());
        assert!(modkey("Mod").is_err());
    }

    #[test]
    fn parses_buttons() {
        assert_eq!(button("Mod-1", MOD4), Ok((ModMask::M4, 1)));
        assert_eq!(button("3", MOD4), Ok((ModMask::default(), 3)));
    }
}
