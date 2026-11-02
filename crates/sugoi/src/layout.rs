//! Geometry for layouts and states (`doc/spec/layouts.md`), apart from X.

/// A window's outer rectangle: position, and size including the border.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect { x, y, width, height }
    }
}

/// [Y7]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Tile,
    Max,
    Float,
}

impl Layout {
    const NAMES: [(Layout, &'static str); 3] =
        [(Layout::Tile, "tile"), (Layout::Max, "max"), (Layout::Float, "float")];

    /// [Y7] The name the config and the kept state ([V11]) use.
    pub fn name(self) -> &'static str {
        Layout::NAMES.iter().find(|(l, _)| *l == self).map_or("tile", |(_, n)| n)
    }

    pub fn from_name(name: &str) -> Option<Layout> {
        Layout::NAMES.iter().find(|(_, n)| *n == name).map(|(l, _)| *l)
    }
}

/// [Y2]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// [Y2] [Y4] The master column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Master {
    pub side: Side,
    pub width: f32,
}

impl Default for Master {
    fn default() -> Self {
        Master { side: Side::Right, width: 0.5 }
    }
}

/// Bounds of the master width and of a share ([Y3], [Y4]).
pub const MIN_FACTOR: f32 = 0.05;
pub const MAX_FACTOR: f32 = 0.95;

/// [Y2] [Y3] The cells of `shares.len() + 1` tiled windows: the master, then the stack.
pub fn tile(area: Rect, master: Master, shares: &[f32]) -> Vec<Rect> {
    if shares.is_empty() {
        return vec![area];
    }
    let factor = master.width.clamp(MIN_FACTOR, MAX_FACTOR);
    let master_width = (area.width as f32 * factor).round() as u32;
    let stack_width = area.width - master_width;
    let (master_x, stack_x) = match master.side {
        Side::Left => (area.x, area.x + master_width as i32),
        Side::Right => (area.x + stack_width as i32, area.x),
    };
    let mut cells = vec![Rect::new(master_x, area.y, master_width, area.height)];
    // Round the boundaries, not the heights: cells then fill the column exactly and can
    // never pass its end, whatever the rounding of each.
    let total: f32 = shares.iter().sum();
    let mut sum = 0.0;
    let mut top = 0u32;
    for (index, share) in shares.iter().enumerate() {
        sum += share;
        // The last cell ends the column, whatever the float error in its boundary.
        let bottom = if index + 1 == shares.len() {
            area.height
        } else {
            ((area.height as f32 * sum / total).round() as u32).min(area.height)
        };
        cells.push(Rect::new(stack_x, area.y + top as i32, stack_width, bottom - top));
        top = bottom;
    }
    cells
}

/// [Y3] Splits `total` of the column among `weights` in proportion, with no part below
/// `MIN_FACTOR`: parts that would be are set to it, and the rest shared again.
fn spread(weights: &[f32], total: f32) -> Vec<f32> {
    let mut parts = vec![0.0; weights.len()];
    let mut free: Vec<usize> = (0..weights.len()).collect();
    let mut left = total;
    while !free.is_empty() {
        let sum: f32 = free.iter().map(|&i| weights[i]).sum();
        let small: Vec<usize> =
            free.iter().copied().filter(|&i| left * weights[i] / sum < MIN_FACTOR).collect();
        if small.is_empty() {
            for &i in &free {
                parts[i] = left * weights[i] / sum;
            }
            break;
        }
        for &i in &small {
            parts[i] = MIN_FACTOR;
            left -= MIN_FACTOR;
        }
        free.retain(|i| !small.contains(i));
    }
    parts
}

/// [Y3] Rescales `weights` to average 1.0, each at least `MIN_FACTOR` of their sum, so
/// a new window's 1.0 is an average share whatever came and went.
pub fn balance(weights: &mut [f32]) {
    let count = weights.len() as f32;
    let parts = spread(weights, 1.0);
    for (weight, part) in weights.iter_mut().zip(parts) {
        *weight = part * count;
    }
}

/// [Y3] Adds `delta` of the column to share `index`; the others share the rest in
/// proportion, none below `MIN_FACTOR`. The weights stay balanced.
pub fn grow_share(weights: &mut [f32], index: usize, delta: f32) {
    let count = weights.len();
    if count < 2 {
        return;
    }
    let parts = spread(weights, 1.0);
    let most = (1.0 - MIN_FACTOR * (count - 1) as f32).min(MAX_FACTOR);
    let target = (parts[index] + delta).clamp(MIN_FACTOR, most.max(MIN_FACTOR));
    let others: Vec<f32> = (0..count).filter(|&i| i != index).map(|i| parts[i]).collect();
    let mut rest = spread(&others, 1.0 - target).into_iter();
    for (i, weight) in weights.iter_mut().enumerate() {
        let part = if i == index { target } else { rest.next().expect("one part per other") };
        *weight = part * count as f32;
    }
}

/// [G3] Sets share `index` so that its edge sits at `edge` (0 to 1 of the column): its
/// bottom edge, or its top one for the last share; the others keep their proportions,
/// so the shares above shrink or grow with the rest.
pub fn share_to_edge(weights: &mut [f32], index: usize, edge: f32) {
    let count = weights.len();
    if count < 2 {
        return;
    }
    let parts = spread(weights, 1.0);
    let target = if index + 1 == count {
        1.0 - edge
    } else {
        // The parts above scale by (1 - target) / (1 - part): solve top + target = edge.
        let above: f32 = parts[..index].iter().sum();
        let k = above / (1.0 - parts[index]);
        (edge - k) / (1.0 - k)
    };
    grow_share(weights, index, target - parts[index]);
}

/// [G3] The master width that puts the edge between the columns at `x`.
pub fn master_width_at(area: Rect, side: Side, x: i32) -> f32 {
    let fraction = (x - area.x) as f32 / area.width as f32;
    let width = match side {
        Side::Left => fraction,
        Side::Right => 1.0 - fraction,
    };
    width.clamp(MIN_FACTOR, MAX_FACTOR)
}

/// [G3] Where the edge between the columns is: also with one window, the master's alone.
pub fn column_edge(area: Rect, master: Master) -> i32 {
    let master_width = (area.width as f32 * master.width.clamp(MIN_FACTOR, MAX_FACTOR)).round();
    match master.side {
        Side::Left => area.x + master_width as i32,
        Side::Right => area.x + area.width as i32 - master_width as i32,
    }
}

/// [Y8] The window's position and inner size in `cell`, at least 1 px.
pub fn inner(cell: Rect, border: u16) -> (i32, i32, u32, u32) {
    let border = 2 * u32::from(border);
    (
        cell.x,
        cell.y,
        cell.width.saturating_sub(border).max(1),
        cell.height.saturating_sub(border).max(1),
    )
}

/// [W2] [W7] `cell` centered on `screen`.
pub fn center(screen: Rect, cell: Rect) -> Rect {
    let x = screen.x + (screen.width as i32 - cell.width as i32) / 2;
    let y = screen.y + (screen.height as i32 - cell.height as i32) / 2;
    Rect::new(x, y, cell.width, cell.height)
}

/// [W2] `cell` moved to stay on `screen` as far as its size allows.
pub fn on_screen(screen: Rect, cell: Rect) -> Rect {
    let right = screen.x + screen.width as i32 - cell.width as i32;
    let bottom = screen.y + screen.height as i32 - cell.height as i32;
    Rect::new(
        cell.x.min(right).max(screen.x),
        cell.y.min(bottom).max(screen.y),
        cell.width,
        cell.height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect { x: 0, y: 0, width: 1280, height: 800 };

    #[test]
    fn a_middle_share_puts_its_bottom_edge_at_the_pointer() {
        let mut shares = [1.0, 1.0, 1.0];
        share_to_edge(&mut shares, 1, 0.75);
        let cells = tile(AREA, Master::default(), &shares);
        let bottom = cells[2].y + cells[2].height as i32;
        assert!((bottom - 600).abs() <= 1, "{cells:?}");
        // The cell above keeps its proportion to the one below.
        let (above, below) = (cells[1].height as f32, cells[3].height as f32);
        assert!((above / below - 1.0).abs() < 0.01, "{cells:?}");
    }

    #[test]
    fn the_last_share_puts_its_top_edge_at_the_pointer() {
        let mut shares = [1.0, 1.0, 1.0];
        share_to_edge(&mut shares, 2, 0.25);
        let cells = tile(AREA, Master::default(), &shares);
        assert!((cells[3].y - 200).abs() <= 1, "{cells:?}");
    }

    #[test]
    fn edges_keep_the_shares_within_bounds() {
        let mut shares = [1.0, 1.0];
        share_to_edge(&mut shares, 0, 1.0);
        let parts: Vec<f32> = shares.iter().map(|s| s / 2.0).collect();
        assert!(parts.iter().all(|&p| p >= MIN_FACTOR - 1e-4), "{parts:?}");
    }

    #[test]
    fn the_column_edge_follows_the_master_width() {
        let master = Master::default();
        assert_eq!(column_edge(AREA, master), 640);
        assert!((master_width_at(AREA, Side::Right, 320) - 0.75).abs() < 1e-6);
        assert!((master_width_at(AREA, Side::Left, 320) - 0.25).abs() < 1e-6);
        assert_eq!(master_width_at(AREA, Side::Right, 1279), MIN_FACTOR);
    }

    #[test]
    fn one_window_fills_the_area() {
        assert_eq!(tile(AREA, Master::default(), &[]), [AREA]);
    }

    #[test]
    fn the_master_goes_right_by_default() {
        let cells = tile(AREA, Master::default(), &[1.0, 1.0]);
        assert_eq!(
            cells,
            [Rect::new(640, 0, 640, 800), Rect::new(0, 0, 640, 400), Rect::new(0, 400, 640, 400)]
        );
    }

    #[test]
    fn the_master_side_and_width_are_set() {
        let master = Master { side: Side::Left, width: 0.6 };
        let cells = tile(AREA, master, &[1.0]);
        assert_eq!(cells, [Rect::new(0, 0, 768, 800), Rect::new(768, 0, 512, 800)]);
    }

    #[test]
    fn stack_cells_fill_the_column_exactly() {
        let cells = tile(Rect::new(0, 0, 100, 100), Master::default(), &[1.0, 1.0, 1.0]);
        let heights: Vec<u32> = cells[1..].iter().map(|c| c.height).collect();
        assert_eq!(heights, [33, 34, 33]);
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn growing_a_share_takes_from_the_others_in_proportion() {
        let mut weights = [1.5, 0.75, 0.75];
        grow_share(&mut weights, 0, 0.1);
        let parts: Vec<f32> = weights.iter().map(|w| w / 3.0).collect();
        assert!(close(parts[0], 0.6) && close(parts[1], 0.2) && close(parts[2], 0.2));
    }

    #[test]
    fn unequal_shares_keep_their_minimum() {
        let mut weights = [0.6, 2.1, 0.3];
        grow_share(&mut weights, 1, 0.2);
        let parts: Vec<f32> = weights.iter().map(|w| w / 3.0).collect();
        assert!(
            close(parts[1], 0.9) && close(parts[0], 0.05) && close(parts[2], 0.05),
            "{parts:?}"
        );
    }

    #[test]
    fn balancing_brings_weights_back_to_an_average_of_one() {
        let mut weights = [2.7, 0.15, 1.0];
        balance(&mut weights);
        assert!(close(weights.iter().sum(), 3.0));
        assert!(weights.iter().all(|w| w / 3.0 >= MIN_FACTOR - 1e-6));
    }

    #[test]
    fn shares_keep_their_sum_and_their_minimum() {
        let mut shares = [1.0, 1.0, 1.0];
        grow_share(&mut shares, 0, 0.05);
        assert!(close(shares.iter().sum(), 3.0));
        grow_share(&mut shares, 0, 1.0);
        let fractions: Vec<f32> = shares.iter().map(|s| s / 3.0).collect();
        assert!(close(fractions[0], 0.9) && close(fractions[1], 0.05) && close(fractions[2], 0.05));
    }

    #[test]
    fn a_master_width_out_of_range_is_clamped() {
        let master = Master { side: Side::Left, width: 1.2 };
        assert_eq!(tile(AREA, master, &[1.0])[1], Rect::new(1216, 0, 64, 800));
    }

    #[test]
    fn inner_size_leaves_room_for_the_border_and_never_vanishes() {
        assert_eq!(inner(Rect::new(10, 20, 100, 50), 3), (10, 20, 94, 44));
        assert_eq!(inner(Rect::new(0, 0, 4, 4), 3), (0, 0, 1, 1));
    }

    #[test]
    fn centering_and_keeping_on_screen() {
        assert_eq!(center(AREA, Rect::new(0, 0, 280, 200)), Rect::new(500, 300, 280, 200));
        assert_eq!(on_screen(AREA, Rect::new(1200, -50, 200, 100)), Rect::new(1080, 0, 200, 100));
    }
}
