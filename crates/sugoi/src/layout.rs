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
    let total: f32 = shares.iter().sum();
    let mut y = area.y;
    for (index, share) in shares.iter().enumerate() {
        // The last cell takes what rounding left, so the column is exactly filled.
        let height = if index + 1 == shares.len() {
            (area.y + area.height as i32 - y) as u32
        } else {
            (area.height as f32 * share / total).round() as u32
        };
        cells.push(Rect::new(stack_x, y, stack_width, height));
        y += height as i32;
    }
    cells
}

/// [Y3] Adds `delta` of the column to share `index`, taking it from the others in
/// proportion, so each keeps at least `MIN_FACTOR` of the column. Shares are weights:
/// their sum stays what it was, so a new window's 1.0 is an average share.
pub fn grow_share(shares: &mut [f32], index: usize, delta: f32) {
    let total: f32 = shares.iter().sum();
    let fraction = |share: f32| share / total;
    let others = 1.0 - fraction(shares[index]);
    if others <= 0.0 {
        return;
    }
    let most = (1.0 - MIN_FACTOR * (shares.len() - 1) as f32).min(MAX_FACTOR);
    let target = (fraction(shares[index]) + delta).clamp(MIN_FACTOR, most.max(MIN_FACTOR));
    let scale = (1.0 - target) / others;
    for (i, share) in shares.iter_mut().enumerate() {
        let new = if i == index { target } else { fraction(*share) * scale };
        *share = new * total;
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
        assert_eq!(heights, [33, 33, 34]);
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn growing_a_share_takes_from_the_others_in_proportion() {
        let mut shares = [0.5, 0.25, 0.25];
        grow_share(&mut shares, 0, 0.1);
        assert!(close(shares[0], 0.6) && close(shares[1], 0.2) && close(shares[2], 0.2));
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
