//! Canonical units of the kernel: lengths in millimetres, angles in radians.
//! The UI may display metres or inches; nothing inside the engine ever does.

/// Geometric tolerance for coincidence tests (planes touching, AABB contact).
pub const EPS: f64 = 1e-6;

/// Output precision. Every length written to a plan is rounded here so that
/// floating-point noise never shows up as a diff in a regression fixture.
pub fn round3(v: f64) -> f64 {
    let r = (v * 1000.0).round() / 1000.0;
    // Normalise -0.0 so serialised output is stable.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

pub fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_to_micron() {
        assert_eq!(round3(1.23456), 1.235);
        assert_eq!(round3(-0.0001), 0.0);
        assert_eq!(round3(9.0), 9.0);
    }
}
