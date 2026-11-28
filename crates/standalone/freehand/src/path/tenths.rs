//! Numbers as path data writes them: whole tenths of a pixel. The pencil's ink is drawn without
//! anti-aliasing, so a finer step never shows, and every digit saved is a byte on every page.

use alloc::string::String;

/// A coordinate in whole tenths of a pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tenths(pub(crate) i64);

/// The largest magnitude written, in pixels: far past any drawing, and small enough that every
/// sum of tenths stays exact.
const LARGEST: f64 = 1.0e12;

impl Tenths {
    /// `x` in tenths, the nearest whole number of them, halves away from zero. `None` past
    /// [`LARGEST`] or when not a number.
    pub(crate) fn of(x: f64) -> Option<Self> {
        if x.is_nan() || libm::fabs(x) > LARGEST {
            return None;
        }
        Some(Self::nearest(x * 10.0))
    }

    /// The tenths nearest `value`, already in tenths: halves away from zero; `value` is finite
    /// and below 2^62.
    pub(crate) fn nearest(value: f64) -> Self {
        #[expect(clippy::cast_possible_truncation, reason = "finite and well inside i64")]
        let whole = value as i64;
        #[expect(clippy::cast_precision_loss, reason = "below 2^53, so exact")]
        let rest = value - whole as f64;
        Self(if rest >= 0.5 {
            whole + 1
        } else if rest <= -0.5 {
            whole - 1
        } else {
            whole
        })
    }

    /// Appends it: `12.5`, `-3`, `.5`, `-.5`; zero is `0`. A leading zero and a trailing `.0`
    /// are left out, as SVG's number grammar allows. The digits go in a character at a time:
    /// measured faster than formatting into a buffer and appending it, which pays for a UTF-8
    /// check and a copy.
    pub(crate) fn push_to(self, out: &mut String) {
        let magnitude = self.0.unsigned_abs();
        let (units, tenth) = (magnitude / 10, magnitude % 10);
        if self.0 < 0 {
            out.push('-');
        }
        if units > 0 || tenth == 0 {
            let mut digits = [0u8; 20];
            let mut at = digits.len();
            let mut rest = units;
            loop {
                at -= 1;
                if let Some(slot) = digits.get_mut(at) {
                    *slot = b'0' + u8::try_from(rest % 10).unwrap_or(0);
                }
                rest /= 10;
                if rest == 0 || at == 0 {
                    break;
                }
            }
            for &digit in digits.get(at..).unwrap_or_default() {
                out.push(char::from(digit));
            }
        }
        if tenth > 0 {
            out.push('.');
            out.push(char::from(b'0' + u8::try_from(tenth).unwrap_or(0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::Tenths;

    fn written(x: f64) -> String {
        let mut out = String::new();
        if let Some(t) = Tenths::of(x) {
            t.push_to(&mut out);
        }
        out
    }

    #[test]
    fn tenths_are_short() {
        assert_eq!(written(12.5), "12.5");
        assert_eq!(written(12.0), "12");
        assert_eq!(written(0.5), ".5");
        assert_eq!(written(-0.5), "-.5");
        assert_eq!(written(-0.04), "0");
        assert_eq!(written(0.0), "0");
        assert_eq!(written(0.25), ".3");
        assert_eq!(written(-0.25), "-.3");
        assert_eq!(written(1234.96), "1235");
        assert_eq!(written(f64::NAN), "");
        assert_eq!(written(1.0e13), "");
    }
}
