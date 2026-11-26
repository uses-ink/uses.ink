//! Numbers as SVG path data writes them: two decimals, rounded as a browser's `toFixed(2)`
//! rounds, so a path is the same text wherever it is drawn.

use alloc::string::String;
use core::fmt::{self, Write as _};

/// A coordinate written with two decimals. A value exactly halfway rounds away from zero, and
/// negative zero is written `0.00`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixed(pub(crate) f64);

/// Below this, `x * 100` rounded to a whole number fits a `u64` exactly and the fast path holds.
const FAST: f64 = 9.0e15;

impl Fixed {
    /// Appends the number to `out`: the fast path for every coordinate a drawing has.
    pub(crate) fn push_to(self, out: &mut String) {
        let x = self.0;
        let magnitude = libm::fabs(x);
        if magnitude >= FAST || magnitude.is_nan() {
            // huge or not a number: the general formatter, which a `String` never refuses
            let _infallible = write!(out, "{self}");
            return;
        }
        if x < 0.0 {
            out.push('-');
        }
        push_hundredths(out, hundredths(magnitude));
    }
}

/// `x * 100` rounded to a whole number, a tie away from zero, decided on the exact product.
///
/// `n` is `floor(h)`. The rounded product `h` is within `h * 2^-53` of the exact one, so when `h` is further than
/// that from a half the half it is on is the answer. `floor(h)` may be one off near a whole
/// number, which does not change the rounding there. Only near a half does `fma(x, 100, -(n +
/// 0.5))` decide: `fma` rounds once, so its sign is the sign of the exact difference, and it is
/// zero only on an exact tie. `fma` is software in `libm`, so it runs only there.
fn hundredths(x: f64) -> u64 {
    let h = x * 100.0;
    // `floor` of a non-negative number below 2^63 is truncation: one instruction where
    // `libm::floor` is a software routine
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative number below FAST * 100, so below 2^63"
    )]
    let whole = h as u64;
    #[expect(clippy::cast_precision_loss, reason = "below 2^53, so exact")]
    let n = whole as f64;
    let off = h - n - 0.5;
    let margin = h * 4.0 * f64::EPSILON;
    let up = if off > margin {
        true
    } else if off < -margin {
        false
    } else {
        libm::fma(x, 100.0, -(n + 0.5)) >= 0.0
    };
    whole + u64::from(up)
}

/// Writes `n` hundredths as `units.hh`, a character at a time: measured faster than building a
/// buffer and appending it, which pays for a UTF-8 check and a copy.
fn push_hundredths(out: &mut String, n: u64) {
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut units = n / 100;
    loop {
        at -= 1;
        let digit = (units % 10) as u8;
        if let Some(slot) = digits.get_mut(at) {
            *slot = b'0' + digit;
        }
        units /= 10;
        if units == 0 || at == 0 {
            break;
        }
    }
    for &digit in digits.get(at..).unwrap_or_default() {
        out.push(char::from(digit));
    }
    let cents = (n % 100) as u8;
    out.push('.');
    out.push(char::from(b'0' + cents / 10));
    out.push(char::from(b'0' + cents % 10));
}

impl fmt::Display for Fixed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = self.0;
        if libm::fabs(x) < FAST {
            let mut text = String::new();
            self.push_to(&mut text);
            return out.write_str(&text);
        }
        if x < 0.0 {
            out.write_str("-")?;
        }
        write!(out, "{:.2}", libm::fabs(x))
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
        assert_eq!(Fixed(0.005).to_string(), "0.01"); // every expected value here is what a browser's `toFixed(2)` writes
        assert_eq!(Fixed(1.005).to_string(), "1.00");
        assert_eq!(Fixed(1_234_567.895).to_string(), "1234567.90"); // as `toFixed(2)` writes it
        assert_eq!(Fixed(0.995).to_string(), "0.99");
        assert_eq!(Fixed(9.995).to_string(), "9.99");
        assert_eq!(Fixed(99.995).to_string(), "100.00");
    }

    /// The fast path agrees with exact decimal formatting on every non-tie, over many values.
    #[test]
    fn fast_path_matches_exact_formatting() {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..20_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            #[expect(clippy::cast_precision_loss, reason = "test values")]
            let x = ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 2.0e4;
            let exact = alloc::format!("{:.2}", libm::fabs(x));
            let fast = Fixed(libm::fabs(x)).to_string();
            // `{:.2}` rounds an exact tie to even; only ties may differ, and they are checked above
            let tie = libm::fma(libm::fabs(x), 100.0, -(libm::floor(libm::fabs(x) * 100.0) + 0.5)) == 0.0;
            if !tie {
                assert_eq!(fast, exact, "{x:e}");
            }
        }
    }
}
