//! Slanted pixel wordmark adapted from Osdy-Pi's `SLANTED_PIXEL_ROWS`.

/// "Osdy Code" in logical pixels: `C` bevel, `S` face, `V` depth, `.` transparent.
pub const OSDY_CODE: &[&str] = &[
    ".........................................................................................................................",
    "......CCCC........................CCCCC........................CCCC................................CCCCC.................",
    "....CCCSSSCCC.....................CSSSS......................CCCSSSCCC.............................CSSSS.................",
    "..CCCSSSSSSSSC....................CSSSVV...................CCCSSSSSSSSC............................CSSSVV................",
    "..CSSSSSSSSSSSC...................CSSSVV...................CSSSSSSSSSSSC...........................CSSSVV................",
    ".CCSSSSSSSSSSSS...CCCCCC.....CCCC.CSSSVCCCCC...CCCC.......CCSSSSSSSSSSSV......CCCCCC..........CCCC.CSSSV......CCCCCC.....",
    "CCSSSSVVVVSSSSSV.CCSSSSSC..CCCSSSCCSSSVCSSSS..CCSSS......CCSSSSVVVVSSVVV....CCCSSSSSCC......CCCSSSCCSSSV....CCCSSSSSCC...",
    "CSSSSVVVVVCSSSSVCCSSSSSSV.CCSSSSSSSSSSV.SSSSV.CSSSVV.....CSSSSVVVVVVVVV....CCSSSSSSSSSC....CCSSSSSSSSSSV...CCSSSSSSSSSC..",
    "CSSSSVV...CSSSSVCSSSVVVVVVCSSSSVSSSSSSV.CSSSVCCSSSV......CSSSSVV...........CSSSSVVCSSSSV...CSSSSVSSSSSSV...CSSSVV.CSSSSV.",
    "CSSSSV....CSSSSVCSSSCCCVV.CSSSVVVSSSSVV.CSSSVCSSSVV......CSSSSV............CSSSVV.CSSSVV...CSSSVVVSSSSVV...CSSSCCCCSSSVV.",
    "CSSSSV....CSSSSV.SSSSSSC.CCSSSVV.CSSSVV.CSSSCCSSSVV......CSSSSV.....CCC...CCSSSV..CSSSVV..CCSSSVV.CSSSVV..CCSSSSSSSSSVVV.",
    "CSSSSC..CCCSSSVV.VVSSSSS.CSSSSV.CCSSSV..CSSSSSSSVV.......CSSSSC..CCCSSSV..CSSSSV.CCSSSV...CSSSSV.CCSSSV...CSSSSVVVVVV....",
    ".SSSSSCCCSSSSSVVCCVVSSSSVCSSSSCCCSSSSV...SSSSSSSVV........SSSSSCCCSSSSSVV.CSSSSCCCSSSSV...CSSSSCCCSSSSV...CSSSSV...CCCC..",
    ".CSSSSSSSSSSSVVCCSCCCSSSV.SSSSSSSSSSSV...CSSSSSVV.........CSSSSSSSSSSSVV...SSSSSSSSSSVV....SSSSSSSSSSSV....SSSSSCCCSSSSV.",
    "..SSSSSSSSSSVVVCSSSSSSSVV.CSSSSSSSSSSV...CSSSSSVV..........SSSSSSSSSSVVV...CSSSSSSSSVVV....CSSSSSSSSSSV....CSSSSSSSSSVV..",
    "..VVSSSSSVVVVV..VSSSSVVVV..VSSSVVVVVVV...CSSSSVV...........VVSSSSSVVVVV.....VSSSVVVVVV......VSSSVVVVVVV.....VSSSVVVVVV...",
    "...VVVVVVVVVV...VVVVVVVV...VVVVVVVVVVV.CCCSSSVVV............VVVVVVVVVV......VVVVVVVVV.......VVVVVVVVVVV.....VVVVVVVVV....",
    ".....VVVVV........VVVV.......VVV......CCSSSSSVV...............VVVVV...........VVV.............VVV.............VVV........",
    "......................................CSSSSSVV...........................................................................",
    "......................................CSSSSVVV...........................................................................",
    ".......................................VVVVVV............................................................................",
    ".......................................VVVVV.............................................................................",
];

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
    use super::{OSDY_CODE, Role, pixels, size};

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

    #[test]
    fn three_blank_columns_separate_the_words() {
        let (width, _) = size();
        let width = usize::from(width);
        assert!((width / 3..(2 * width / 3).saturating_sub(2)).any(|x| {
            OSDY_CODE
                .iter()
                .all(|row| &row.as_bytes()[x..x + 3] == b"...")
        }));
    }
}
