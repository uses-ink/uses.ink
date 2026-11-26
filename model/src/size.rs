//! Sizes in bytes.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A number of bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bytes(pub u64);

impl Bytes {
    /// How many bytes.
    pub const fn count(self) -> u64 {
        self.0
    }
}

impl From<u64> for Bytes {
    fn from(count: u64) -> Self {
        Self(count)
    }
}

impl fmt::Display for Bytes {
    /// The largest binary unit the size reaches, with one decimal truncated toward zero, so
    /// `26_214_400` is `25 MiB` and `1536` is `1.5 KiB`. Integer arithmetic only.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: [&str; 6] = ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
        let mut scale: u64 = 1;
        let mut unit = None;
        for name in UNITS {
            match scale.checked_mul(1024) {
                Some(next) if self.0 >= next => {
                    scale = next;
                    unit = Some(name);
                }
                _ => break,
            }
        }
        let Some(unit) = unit else {
            return if self.0 == 1 { write!(out, "1 byte") } else { write!(out, "{} bytes", self.0) };
        };
        let whole = self.0 / scale;
        let tenth = (self.0 % scale) * 10 / scale;
        if tenth == 0 { write!(out, "{whole} {unit}") } else { write!(out, "{whole}.{tenth} {unit}") }
    }
}

#[cfg(test)]
mod test {
    use super::Bytes;

    #[test]
    fn shows_the_largest_unit() {
        let shown = |n: u64| Bytes(n).to_string();
        assert_eq!(shown(0), "0 bytes");
        assert_eq!(shown(1), "1 byte");
        assert_eq!(shown(1023), "1023 bytes");
        assert_eq!(shown(1536), "1.5 KiB");
        assert_eq!(shown(25 * 1024 * 1024), "25 MiB");
        assert_eq!(shown(u64::MAX), "15.9 EiB");
    }
}
