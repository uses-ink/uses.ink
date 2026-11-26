//! Numbers as SVG path data writes them: two decimals, rounded as a browser's `toFixed(2)`
//! rounds, so a path is the same text wherever it is drawn.

use core::fmt;

/// A coordinate written with two decimals. A value exactly halfway rounds away from zero, and
/// negative zero is written `0.00`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixed(pub(crate) f64);

impl fmt::Display for Fixed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = self.0;
        if x < 0.0 {
            out.write_str("-")?;
        }
        let x = libm::fabs(x);
        let hundredths = x * 100.0;
        // An exact tie: the product is exact and lands on a half.
        let exact = libm::fma(x, 100.0, -hundredths) == 0.0;
        #[expect(clippy::float_cmp, reason = "a tie is exactly a half, never nearly one")]
        let tie = exact && hundredths - libm::floor(hundredths) == 0.5;
        if tie {
            // A tie is a whole number of halves below 2^53, so it fits.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a non-negative whole number below 2^53"
            )]
            let n = (libm::floor(hundredths) + 1.0) as u64;
            return write!(out, "{}.{:02}", n / 100, n % 100);
        }
        write!(out, "{x:.2}")
    }
}

/// `Math.round`: halves round up.
pub(crate) fn round(x: f64) -> f64 {
    let floor = libm::floor(x);
    if x - floor >= 0.5 { floor + 1.0 } else { floor }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::Fixed;

    #[test]
    fn ties_round_away_from_zero() {
        assert_eq!(Fixed(0.125).to_string(), "0.13");
        assert_eq!(Fixed(-0.125).to_string(), "-0.13");
        assert_eq!(Fixed(2.675).to_string(), "2.67"); // not a tie: 2.67499999..
        assert_eq!(Fixed(-0.0).to_string(), "0.00");
        assert_eq!(Fixed(-0.001).to_string(), "-0.00");
        assert_eq!(Fixed(35.2).to_string(), "35.20");
    }
}
