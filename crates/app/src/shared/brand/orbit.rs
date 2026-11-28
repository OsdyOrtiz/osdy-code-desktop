pub(super) struct OrbitPoint {
    pub x: f32,
    pub y: f32,
    pub visibility: f32,
}

/// Position in the 384 px badge canvas. `phase` is in turns and may wrap.
/// The path stays fixed while the disc bobs vertically in canvas units.
pub(super) fn position(phase: f32, bob: f32) -> OrbitPoint {
    let (depth, along) = (phase.rem_euclid(1.) * std::f32::consts::TAU).sin_cos();
    let (sin, cos) = (-12_f32).to_radians().sin_cos();
    let (major, minor) = (175.5 * along, 44.5 * depth);
    let x = 191.5 + major * cos - minor * sin;
    let y = 230. + major * sin + minor * cos;

    // Fade the back arc just outside the disc so even its halo disappears
    // behind the rim. The near arc stays bright, including across the face.
    let visibility = if depth < 0. {
        // Match the crop in badge.svg (center 192,192; radius 137).
        let rim = (x - 192.).hypot(y - (192. + bob)) - 137.;
        let edge = (rim / 16.).clamp(0., 1.);
        edge * edge * (3. - 2. * edge)
    } else {
        1.
    };
    OrbitPoint { x, y, visibility }
}

#[cfg(test)]
mod tests {
    fn position(phase: f32) -> super::OrbitPoint {
        super::position(phase, 0.)
    }

    #[test]
    fn follows_the_measured_ring_and_wraps() {
        for (phase, x, y) in [
            (0., 363., 194.),
            (0.25, 201., 274.),
            (0.5, 20., 266.),
            (0.75, 182., 186.),
            (1., 363., 194.),
            (-0.75, 201., 274.),
        ] {
            let p = position(phase);
            assert!(
                (p.x - x).hypot(p.y - y) < 1.,
                "phase {phase}: ({}, {})",
                p.x,
                p.y
            );
        }
    }

    #[test]
    fn back_arc_occlusion_follows_the_disc_without_moving_the_path() {
        let up = super::position(0.61, -8.);
        let down = super::position(0.61, 8.);
        assert_eq!((up.x, up.y), (down.x, down.y));
        assert!(up.visibility > down.visibility);
    }

    #[test]
    fn only_the_far_arc_inside_the_disc_is_hidden() {
        assert_eq!(position(0.).visibility, 1.);
        assert_eq!(position(0.25).visibility, 1.);
        assert_eq!(position(0.5).visibility, 1.);
        assert_eq!(position(0.75).visibility, 0.);
        for i in 50..=75 {
            let phase = i as f32 / 100.;
            let p = position(phase);
            assert!((0. ..=1.).contains(&p.visibility));
            assert!(p.visibility >= position(phase + 0.001).visibility);
        }
        assert!((51..75).any(
            |i| (0. ..1.).contains(&position(i as f32 / 100.).visibility)
                && position(i as f32 / 100.).visibility > 0.
        ));
    }
}
