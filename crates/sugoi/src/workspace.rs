//! Workspace arithmetic (`doc/spec/workspaces.md`). Indices are 0 to 29, as in EWMH;
//! users and configs number workspaces 1 to 30.

/// [V1]
pub const COUNT: usize = 30;
/// [V1] Workspaces per bank: the keys 1 to 0.
pub const BANK: usize = 10;

/// [V4] [V8] Where `workspace n` goes from `current`, given which workspaces hold a
/// window: for n from 1 to 10, the next workspace of n's class after `current` that
/// is n or `used`, wrapping (`current` when none is), or n from outside the class;
/// for n from 11 to 30, n. `None` for any other n.
pub fn target(current: usize, n: u32, used: impl Fn(usize) -> bool) -> Option<usize> {
    let n = usize::try_from(n).ok().filter(|n| (1..=COUNT).contains(n))?;
    if n > BANK {
        return Some(n - 1);
    }
    let first = n - 1;
    if current % BANK != first {
        return Some(first);
    }
    let next = (1..=COUNT / BANK)
        .map(|step| (current + step * BANK) % COUNT)
        .find(|&w| w == first || used(w));
    Some(next.unwrap_or(current))
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
        let all = |_| true;
        assert_eq!(target(2, 3, all), Some(12));
        assert_eq!(target(12, 3, all), Some(22));
        assert_eq!(target(22, 3, all), Some(2));
        assert_eq!(target(5, 3, all), Some(2));
        assert_eq!(target(9, 10, all), Some(19));
        assert_eq!(target(0, 13, all), Some(12));
        assert_eq!(target(0, 0, all), None);
        assert_eq!(target(0, 31, all), None);
        // From 3, a window on 23 and none on 13: 23, then back to 3.
        let only_23 = |w| w == 22;
        assert_eq!(target(2, 3, only_23), Some(22));
        assert_eq!(target(22, 3, only_23), Some(2));
        // An empty stacked workspace goes on to the next used one, or n.
        assert_eq!(target(12, 3, only_23), Some(22));
        // With neither, it stays; n counts even empty.
        let none = |_| false;
        assert_eq!(target(2, 3, none), Some(2));
        assert_eq!(target(12, 3, none), Some(2));
        assert_eq!(target(5, 3, none), Some(2));
        assert_eq!(target(5, 13, none), Some(12));
        assert_eq!(step(29, true), 0);
        assert_eq!(step(0, false), 29);
    }
}
