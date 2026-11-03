//! Properties of the pure geometry and workspace arithmetic.
//!
//! - **Tile cells** ([Y2], [Y3]): for generated small areas, master columns and stack
//!   weights, every pixel of the area is covered by exactly one cell and none lies
//!   outside it; the master column is the master width of the area, rounded, and each
//!   stack cell within a pixel of its share. The oracle counts pixels, sharing nothing
//!   with `tile`.
//! - **Balance** ([Y3]): weights keep their count as sum, no share below the floor, and
//!   the ratios of shares above it.
//! - **Growing a share** ([Y3]): the grown share reaches its bounded target, the sum
//!   stays, none goes below the floor, and the others keep their order.
//! - **Edges** ([G3]): for any balanced weights, window and edge, the window's edge (its
//!   bottom, its top for the last) moves toward the edge asked and never past it, reaches
//!   it unless a share meets a bound, and the shares keep their sum and floor. The edge is
//!   summed from the weights, not taken from `share_to_edge`.
//! - **Workspace targets** ([V4], [V5]): for generated occupancies, every current
//!   workspace and every n, against a reading of the spec's wording.

mod common;

use common::cases;
use proptest::prelude::*;
use sugoi::layout::{self, MAX_FACTOR, MIN_FACTOR, Master, Rect, Side};
use sugoi::workspace;

const EPSILON: f32 = 1e-4;

fn area() -> impl Strategy<Value = Rect> {
    (-3i32..3, -3i32..3, 1u32..40, 1u32..40).prop_map(|(x, y, w, h)| Rect::new(x, y, w, h))
}

fn master() -> impl Strategy<Value = Master> {
    (any::<bool>(), 5u32..=95).prop_map(|(left, percent)| Master {
        side: if left { Side::Left } else { Side::Right },
        width: percent as f32 / 100.0,
    })
}

/// Weights from a small menu, so equal ones and extremes come up often.
fn weights(least: usize, most: usize) -> impl Strategy<Value = Vec<f32>> {
    let weight = prop::sample::select(vec![0.01f32, 0.05, 0.1, 0.5, 1.0, 1.0, 1.5, 3.0, 20.0]);
    prop::collection::vec(weight, least..=most)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(512)))]

    #[test]
    fn tile_cells_cover_the_area_exactly_once(
        area in area(), master in master(), shares in weights(0, 7)
    ) {
        let cells = layout::tile(area, master, &shares);
        prop_assert_eq!(cells.len(), shares.len() + 1);
        for cell in &cells {
            prop_assert!(cell.x >= area.x && cell.y >= area.y, "{:?} outside {:?}", cell, area);
            prop_assert!(cell.x + cell.width as i32 <= area.x + area.width as i32);
            prop_assert!(cell.y + cell.height as i32 <= area.y + area.height as i32);
        }
        for px in area.x..area.x + area.width as i32 {
            for py in area.y..area.y + area.height as i32 {
                let covering = cells.iter().filter(|c| {
                    (c.x..c.x + c.width as i32).contains(&px)
                        && (c.y..c.y + c.height as i32).contains(&py)
                }).count();
                prop_assert_eq!(covering, 1, "pixel ({}, {}) in {:?}", px, py, cells);
            }
        }
        if !shares.is_empty() {
            let expected = (area.width as f32 * master.width).round() as u32;
            prop_assert_eq!(cells[0].width, expected);
        }
        // Each stack cell is its share of the column, within a pixel of rounding.
        let total: f32 = shares.iter().sum();
        for (cell, share) in cells.iter().skip(1).zip(&shares) {
            let exact = area.height as f32 * share / total;
            // Each boundary is rounded to within half a pixel, so a cell to within one.
            let error = (cell.height as f32 - exact).abs();
            prop_assert!(error <= 1.0 + EPSILON, "{:?} for {}", cell, exact);
        }
    }

    #[test]
    fn balance_keeps_the_sum_the_floor_and_the_ratios(mut shares in weights(1, 8)) {
        let before = shares.clone();
        layout::balance(&mut shares);
        let count = shares.len() as f32;
        prop_assert!((shares.iter().sum::<f32>() - count).abs() < EPSILON * count);
        let parts: Vec<f32> = shares.iter().map(|s| s / count).collect();
        for &part in &parts {
            prop_assert!(part >= MIN_FACTOR - EPSILON, "{:?}", parts);
        }
        // Shares above the floor keep the ratios of their weights.
        let free: Vec<usize> = (0..parts.len()).filter(|&i| parts[i] > MIN_FACTOR + EPSILON).collect();
        for &i in &free {
            for &j in &free {
                let ratio = before[i] / before[j];
                prop_assert!((shares[i] / shares[j] - ratio).abs() < EPSILON * ratio.max(1.0));
            }
        }
    }

    #[test]
    fn growing_a_share_meets_its_bounds_and_keeps_the_order(
        mut shares in weights(2, 8), index in 0usize..8, delta in -1.0f32..1.0
    ) {
        let index = index % shares.len();
        layout::balance(&mut shares);
        let count = shares.len();
        let before: Vec<f32> = shares.iter().map(|s| s / count as f32).collect();
        layout::grow_share(&mut shares, index, delta);
        let after: Vec<f32> = shares.iter().map(|s| s / count as f32).collect();
        prop_assert!((after.iter().sum::<f32>() - 1.0).abs() < EPSILON * count as f32);
        let most = (1.0 - MIN_FACTOR * (count - 1) as f32).min(MAX_FACTOR);
        let target = (before[index] + delta).clamp(MIN_FACTOR, most);
        prop_assert!((after[index] - target).abs() < EPSILON, "{:?} -> {:?}", before, after);
        for &part in &after {
            prop_assert!(part >= MIN_FACTOR - EPSILON, "{:?}", after);
        }
        for i in (0..count).filter(|&i| i != index) {
            for j in (0..count).filter(|&j| j != index) {
                if before[i] < before[j] - EPSILON {
                    prop_assert!(after[i] <= after[j] + EPSILON, "{:?} -> {:?}", before, after);
                }
            }
        }
    }
}

/// [G3] Where a share's edge is, as a fraction of the column.
fn edge_of(shares: &[f32], index: usize) -> f32 {
    let sum: f32 = shares.iter().sum();
    let end = if index + 1 == shares.len() { index } else { index + 1 };
    shares[..end].iter().sum::<f32>() / sum
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(512)))]

    #[test]
    fn an_edge_moves_toward_the_pointer_and_reaches_it_within_bounds(
        mut shares in weights(2, 8), index in 0usize..8, edge in 0.0f32..=1.0
    ) {
        let index = index % shares.len();
        layout::balance(&mut shares);
        let count = shares.len();
        let old = edge_of(&shares, index);
        layout::share_to_edge(&mut shares, index, edge);
        let new = edge_of(&shares, index);
        let parts: Vec<f32> = shares.iter().map(|s| s / count as f32).collect();
        prop_assert!((parts.iter().sum::<f32>() - 1.0).abs() < EPSILON * count as f32);
        prop_assert!(parts.iter().all(|&p| p >= MIN_FACTOR - EPSILON), "{:?}", parts);
        // Toward the edge, never past it.
        prop_assert!((new - edge).abs() <= (old - edge).abs() + EPSILON, "{old} {new} {edge}");
        prop_assert!((new - old) * (edge - old) >= -EPSILON, "{old} {new} {edge}");
        let most = (1.0 - MIN_FACTOR * (count - 1) as f32).min(MAX_FACTOR);
        let bounded = parts.iter().any(|&p| p < MIN_FACTOR + EPSILON)
            || parts[index] > most - EPSILON;
        prop_assert!(bounded || (new - edge).abs() < 10.0 * EPSILON, "{old} {new} {edge}");
    }
}

/// The spec's [V4] and [V5], word for word: n's class is n, n+10 and n+20; from a
/// workspace in the class, the next in that order, wrapping, that holds a window or is
/// n; none, and it stays; from outside the class, n. Workspaces numbered 1 to 30.
fn spec_target(current: usize, n: u32, used: &[bool; 30]) -> Option<usize> {
    let current = current + 1;
    let n = n as usize;
    match n {
        1..=10 => {
            let class = [n, n + 10, n + 20];
            let goes = match class.iter().position(|&w| w == current) {
                Some(i) => (1..=2)
                    .map(|k| class[(i + k) % 3])
                    .find(|&w| w == n || used[w - 1])
                    .unwrap_or(current),
                None => n,
            };
            Some(goes - 1)
        }
        11..=30 => Some(n - 1),
        _ => None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    // Any occupancy, mostly sparse so empty stacked workspaces are common; every
    // current workspace and every n.
    #[test]
    fn every_workspace_target_matches_the_spec(
        used in prop::array::uniform30(prop::bool::weighted(0.3))
    ) {
        for current in 0..workspace::COUNT {
            for n in 0..=40 {
                let target = workspace::target(current, n, |w| used[w]);
                prop_assert_eq!(target, spec_target(current, n, &used), "{} {}", current, n);
            }
        }
    }
}

#[test]
fn next_and_previous_wrap() {
    for current in 0..workspace::COUNT {
        let next = if current == 29 { 0 } else { current + 1 };
        let previous = if current == 0 { 29 } else { current - 1 };
        assert_eq!(workspace::step(current, true), next);
        assert_eq!(workspace::step(current, false), previous);
    }
}
