include!("raccoon_dots.rs");

type Dot = (usize, usize, u8);

pub(super) fn raccoon(wink: bool) -> (impl Iterator<Item = Dot>, (usize, usize)) {
    let dots = RACCOON.iter().enumerate().flat_map(move |(y, row)| {
        row.iter().enumerate().filter_map(move |(x, &cell)| {
            // Replace the viewer-left eye in the generated art with a single
            // line near its lower edge, leaving the other eye open.
            let shade = if wink && (18..32).contains(&x) && (35..47).contains(&y) {
                if y == 44 && (19..31).contains(&x) {
                    4
                } else {
                    0
                }
            } else {
                cell - b'0'
            };
            (shade != 0).then_some((x, y, shade))
        })
    });
    (dots, (RACCOON[0].len(), RACCOON.len()))
}

#[cfg(test)]
mod tests {
    use super::raccoon;

    #[test]
    fn the_wink_replaces_only_the_left_eye_with_a_flat_line() {
        let frame = |wink| {
            let (dots, (w, h)) = raccoon(wink);
            let mut grid = vec![0; w * h];
            for (x, y, shade) in dots {
                grid[y * w + x] = shade;
            }
            grid
        };
        let open = frame(false);
        let wink = frame(true);
        for y in 0..68 {
            for x in 0..80 {
                let i = y * 80 + x;
                let expected = if (18..32).contains(&x) && (35..47).contains(&y) {
                    if y == 44 && (19..31).contains(&x) {
                        4
                    } else {
                        0
                    }
                } else {
                    open[i]
                };
                assert_eq!(wink[i], expected, "({x}, {y})");
            }
        }
    }

    #[test]
    fn decodes_a_unique_bounded_grid_with_shading() {
        let (dots, (w, h)) = raccoon(false);
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
