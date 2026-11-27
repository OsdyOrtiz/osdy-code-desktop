//! Slanted pixel wordmark adapted from Osdy-Pi's `SLANTED_PIXEL_ROWS`.

/// "Osdy Code_" in logical pixels: `C` bevel, `S` face, `V` depth, `.` transparent.
pub const OSDY_CODE: &[&str] = &[
    "............................................................................................................................................",
    "......CCCC........................CCCCC........................CCCC................................CCCCC....................................",
    "....CCCSSSCCC.....................CSSSS......................CCCSSSCCC.............................CSSSS....................................",
    "..CCCSSSSSSSSC....................CSSSVV...................CCCSSSSSSSSC............................CSSSVV...................................",
    "..CSSSSSSSSSSSC...................CSSSVV...................CSSSSSSSSSSSC...........................CSSSVV...................................",
    ".CCSSSSSSSSSSSS...CCCCCC.....CCCC.CSSSVCCCCC...CCCC.......CCSSSSSSSSSSSV......CCCCCC..........CCCC.CSSSV......CCCCCC........................",
    "CCSSSSVVVVSSSSSV.CCSSSSSC..CCCSSSCCSSSVCSSSS..CCSSS......CCSSSSVVVVSSVVV....CCCSSSSSCC......CCCSSSCCSSSV....CCCSSSSSCC......................",
    "CSSSSVVVVVCSSSSVCCSSSSSSV.CCSSSSSSSSSSV.SSSSV.CSSSVV.....CSSSSVVVVVVVVV....CCSSSSSSSSSC....CCSSSSSSSSSSV...CCSSSSSSSSSC.....................",
    "CSSSSVV...CSSSSVCSSSVVVVVVCSSSSVSSSSSSV.CSSSVCCSSSV......CSSSSVV...........CSSSSVVCSSSSV...CSSSSVSSSSSSV...CSSSVV.CSSSSV....................",
    "CSSSSV....CSSSSVCSSSCCCVV.CSSSVVVSSSSVV.CSSSVCSSSVV......CSSSSV............CSSSVV.CSSSVV...CSSSVVVSSSSVV...CSSSCCCCSSSVV....................",
    "CSSSSV....CSSSSV.SSSSSSC.CCSSSVV.CSSSVV.CSSSCCSSSVV......CSSSSV.....CCC...CCSSSV..CSSSVV..CCSSSVV.CSSSVV..CCSSSSSSSSSVVV....................",
    "CSSSSC..CCCSSSVV.VVSSSSS.CSSSSV.CCSSSV..CSSSSSSSVV.......CSSSSC..CCCSSSV..CSSSSV.CCSSSV...CSSSSV.CCSSSV...CSSSSVVVVVV.......................",
    ".SSSSSCCCSSSSSVVCCVVSSSSVCSSSSCCCSSSSV...SSSSSSSVV........SSSSSCCCSSSSSVV.CSSSSCCCSSSSV...CSSSSCCCSSSSV...CSSSSV...CCCC.....................",
    ".CSSSSSSSSSSSVVCCSCCCSSSV.SSSSSSSSSSSV...CSSSSSVV.........CSSSSSSSSSSSVV...SSSSSSSSSSVV....SSSSSSSSSSSV....SSSSSCCCSSSSV......CCCCCCCCCCCC..",
    "..SSSSSSSSSSVVVCSSSSSSSVV.CSSSSSSSSSSV...CSSSSSVV..........SSSSSSSSSSVVV...CSSSSSSSSVVV....CSSSSSSSSSSV....CSSSSSSSSSVV......CSSSSSSSSSSSSV.",
    "..VVSSSSSVVVVV..VSSSSVVVV..VSSSVVVVVVV...CSSSSVV...........VVSSSSSVVVVV.....VSSSVVVVVV......VSSSVVVVVVV.....VSSSVVVVVV......CSSSSSSSSSSSSVV.",
    "...VVVVVVVVVV...VVVVVVVV...VVVVVVVVVVV.CCCSSSVVV............VVVVVVVVVV......VVVVVVVVV.......VVVVVVVVVVV.....VVVVVVVVV.......VVVVVVVVVVVVVV..",
    ".....VVVVV........VVVV.......VVV......CCSSSSSVV...............VVVVV...........VVV.............VVV.............VVV..........VVVVVVVVVVVVVV...",
    "......................................CSSSSSVV..............................................................................................",
    "......................................CSSSSVVV..............................................................................................",
    ".......................................VVVVVV...............................................................................................",
    ".......................................VVVVV................................................................................................",
];

/// First column of "Code_": the wordmark is drawn in two tones, one per word.
pub const CODE_AT: u16 = 57;

/// Neon tones as `(bevel, face, depth)`: pink for "Osdy", purple for "Code_".
pub const TONES: [(u32, u32, u32); 2] = [
    (0xffd1ec, 0xff6ac1, 0x8c2f73),
    (0xeadcff, 0xb97cff, 0x5b3699),
];

/// Whether column `x` belongs to "Code_" (the second tone) rather than "Osdy".
pub fn second_word(x: u16) -> bool {
    x >= CODE_AT
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Bevel,
    Face,
    Depth,
}

/// Decode the visible pixels in row order, with `(0, 0)` at the upper left.
pub fn pixels() -> Vec<(u16, u16, Role)> {
    OSDY_CODE
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            row.bytes().enumerate().filter_map(move |(x, byte)| {
                let role = match byte {
                    b'C' => Role::Bevel,
                    b'S' => Role::Face,
                    b'V' => Role::Depth,
                    b'.' => return None,
                    _ => unreachable!("invalid wordmark pixel at ({x}, {y})"),
                };
                Some((x as u16, y as u16, role))
            })
        })
        .collect()
}

/// Width and height in logical pixels.
pub fn size() -> (u16, u16) {
    (OSDY_CODE[0].len() as u16, OSDY_CODE.len() as u16)
}

#[cfg(test)]
mod tests {
    use super::{CODE_AT, OSDY_CODE, Role, pixels, second_word, size};

    #[test]
    fn grid_has_equal_width_rows_and_even_height() {
        let (width, height) = size();
        assert!(width > 0);
        assert_eq!(height, 22);
        assert_eq!(height % 2, 0);
        assert_eq!(usize::from(height), OSDY_CODE.len());
        assert!(OSDY_CODE.iter().all(|row| row.len() == usize::from(width)));
    }

    #[test]
    fn pixels_decode_coordinates_and_roles_without_transparency() {
        let mut expected = Vec::new();
        for (y, row) in OSDY_CODE.iter().enumerate() {
            for (x, byte) in row.bytes().enumerate() {
                let role = match byte {
                    b'C' => Role::Bevel,
                    b'S' => Role::Face,
                    b'V' => Role::Depth,
                    b'.' => continue,
                    _ => panic!("invalid wordmark pixel at ({x}, {y})"),
                };
                expected.push((x as u16, y as u16, role));
            }
        }
        let actual = pixels();
        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual, expected);
    }

    fn blank(x: usize) -> bool {
        OSDY_CODE.iter().all(|row| row.as_bytes()[x] == b'.')
    }

    #[test]
    fn the_second_word_starts_after_a_gap() {
        let at = usize::from(CODE_AT);
        assert!((at - 3..at).all(blank));
        assert!(!blank(at));
        assert!(second_word(CODE_AT) && !second_word(CODE_AT - 1));
    }

    #[test]
    fn code_ends_with_an_underscore_on_the_baseline() {
        let width = usize::from(size().0);
        let last = (0..width).rev().find(|&x| !blank(x)).unwrap();
        let start = (0..last).rev().find(|&x| blank(x)).unwrap() + 1;
        // A wide, flat bar: as wide as a letter, sitting on the baseline.
        assert!(last - start + 1 >= 12, "{start}..={last}");
        for (y, row) in OSDY_CODE.iter().enumerate() {
            let inked = row.as_bytes()[start..=last].iter().any(|&b| b != b'.');
            assert_eq!(inked, (13..=17).contains(&y), "row {y}");
        }
    }
}
