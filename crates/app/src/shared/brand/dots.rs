include!("raccoon_dots.rs");

type Dot = (usize, usize, u8);

pub(super) fn raccoon() -> (impl Iterator<Item = Dot>, (usize, usize)) {
    let dots = RACCOON.iter().enumerate().flat_map(|(y, row)| {
        row.iter()
            .enumerate()
            .filter(|&(_, &cell)| cell != b'0')
            .map(move |(x, cell)| (x, y, cell - b'0'))
    });
    (dots, (RACCOON[0].len(), RACCOON.len()))
}

#[cfg(test)]
mod tests {
    use super::raccoon;

    #[test]
    fn decodes_a_unique_bounded_grid_with_shading() {
        let (dots, (w, h)) = raccoon();
        assert_eq!((w, h), (80, 68));
        let mut grid = vec![0; w * h];
        let mut count = 0;
        for (x, y, shade) in dots {
            assert!(x < w && y < h);
            assert!((1..=4).contains(&shade));
            let cell = &mut grid[y * w + x];
            assert_eq!(*cell, 0, "duplicate ({x}, {y})");
            *cell = shade;
            count += 1;
        }
        assert!(count > 1_000 && count < 4_000, "{count} inked cells");
        // Clear outside the head, dark mask between the eyes; bright chin.
        assert_eq!(grid[0], 0);
        assert_eq!(grid[40 * w + 40], 0);
        assert_eq!(grid[60 * w + 40], 4);
        assert!(grid.contains(&1));
    }
}
