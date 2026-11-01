//! Workspace arithmetic (`doc/spec/workspaces.md`). Indices are 0 to 29, as in EWMH;
//! users and configs number workspaces 1 to 30.

/// [V1]
pub const COUNT: usize = 30;
/// [V1] Workspaces per bank: the keys 1 to 0.
pub const BANK: usize = 10;

/// [V4] [V8] Where `workspace n` goes from `current`: for n from 1 to 10, the next
/// workspace of n's class after `current`, wrapping, or n itself from outside the
/// class; for n from 11 to 30, n. `None` for any other n.
pub fn target(current: usize, n: u32) -> Option<usize> {
    let n = usize::try_from(n).ok().filter(|n| (1..=COUNT).contains(n))?;
    if n > BANK {
        return Some(n - 1);
    }
    let first = n - 1;
    if current % BANK == first { Some((current + BANK) % COUNT) } else { Some(first) }
}

/// [V5] The workspace after or before `current`, wrapping.
pub fn step(current: usize, forward: bool) -> usize {
    if forward { (current + 1) % COUNT } else { (current + COUNT - 1) % COUNT }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spec_examples() {
        assert_eq!(target(2, 3), Some(12));
        assert_eq!(target(12, 3), Some(22));
        assert_eq!(target(22, 3), Some(2));
        assert_eq!(target(5, 3), Some(2));
        assert_eq!(target(0, 10), Some(9));
        assert_eq!(target(9, 10), Some(19));
        assert_eq!(target(0, 13), Some(12));
        assert_eq!(target(0, 0), None);
        assert_eq!(target(0, 31), None);
        assert_eq!(step(29, true), 0);
        assert_eq!(step(0, false), 29);
    }
}
