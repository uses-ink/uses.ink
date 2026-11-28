//! Pre-compressed variants: which a deploy may hold and which a client accepts.

use std::fmt;

use crate::http::condition::ConditionError;

/// A content coding a deploy may hold a variant in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Encoding {
    /// `br`.
    Brotli,
    /// `gzip`.
    Gzip,
}

impl Encoding {
    /// Every coding, the one preferred on a tie first.
    pub const ALL: [Self; 2] = [Self::Brotli, Self::Gzip];

    /// The token in `Accept-Encoding` and `Content-Encoding`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Brotli => "br",
            Self::Gzip => "gzip",
        }
    }
}

impl fmt::Display for Encoding {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.token())
    }
}

/// What a client's `Accept-Encoding` allows, each coding weighted in thousandths.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accepted {
    named: Vec<(Encoding, u16)>,
    any: Option<u16>,
}

impl Accepted {
    /// Reads the header's value. A coding this edge never serves (`deflate`, `zstd`) is not an
    /// error: it is not offered.
    pub(crate) fn read(value: &[u8]) -> Result<Self, ConditionError> {
        let text = std::str::from_utf8(value).map_err(|_| ConditionError::AcceptEncoding { at: 0 })?;
        let mut accepted = Self::default();
        let mut start = 0;
        for item in text.split(',') {
            let at = start;
            start += item.len() + 1;
            if item.trim_matches([' ', '\t']).is_empty() {
                continue;
            }
            let (coding, weight) = Self::item(item).ok_or(ConditionError::AcceptEncoding { at })?;
            match coding {
                "*" => accepted.any = Some(weight),
                token => {
                    if let Some(encoding) = Encoding::ALL.into_iter().find(|e| e.token().eq_ignore_ascii_case(token)) {
                        accepted.named.push((encoding, weight));
                    }
                }
            }
        }
        Ok(accepted)
    }

    /// One `coding;q=0.5`: its token and weight. Whitespace around it is allowed.
    fn item(item: &str) -> Option<(&str, u16)> {
        let (coding, params) = match item.split_once(';') {
            Some((coding, params)) => (coding.trim_matches([' ', '\t']), Some(params.trim_matches([' ', '\t']))),
            None => (item.trim_matches([' ', '\t']), None),
        };
        if coding.is_empty() || !coding.bytes().all(|b| b.is_ascii_alphanumeric() || b"-*".contains(&b)) {
            return None;
        }
        let weight = match params {
            None => 1000,
            Some(params) => Self::weight(params.strip_prefix("q=").or_else(|| params.strip_prefix("Q="))?)?,
        };
        Some((coding, weight))
    }

    /// `0`, `0.5`, `1`, `1.000`: at most three decimals, at most 1.
    fn weight(text: &str) -> Option<u16> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        if fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let thousandths: u16 = format!("{fraction:0<3}").parse().ok()?;
        match whole {
            "0" => Some(thousandths),
            "1" if thousandths == 0 => Some(1000),
            _ => None,
        }
    }

    /// How much `encoding` is wanted: its own weight, else the wildcard's, else not at all.
    pub fn weight_of(&self, encoding: Encoding) -> u16 {
        let named = self.named.iter().find(|(named, _)| *named == encoding).map(|(_, weight)| *weight);
        named.or(self.any).unwrap_or(0)
    }

    /// The wanted coding among those a deploy holds: the heaviest, `br` before `gzip` on a tie.
    pub fn choose(&self, held: impl IntoIterator<Item = Encoding>) -> Option<Encoding> {
        let mut held: Vec<Encoding> = held.into_iter().collect();
        held.sort();
        held.into_iter()
            .filter(|encoding| self.weight_of(*encoding) > 0)
            .max_by_key(|encoding| (self.weight_of(*encoding), std::cmp::Reverse(*encoding)))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn read(text: &str) -> Accepted {
        Accepted::read(text.as_bytes()).unwrap()
    }

    #[test]
    fn weighs_and_chooses() {
        let all = Encoding::ALL;
        assert_eq!(read("gzip, br").choose(all), Some(Encoding::Brotli));
        assert_eq!(read("gzip;q=0.8, br;q=0.5").choose(all), Some(Encoding::Gzip));
        assert_eq!(read("br;q=0, gzip").choose(all), Some(Encoding::Gzip));
        assert_eq!(read("*").choose([Encoding::Gzip]), Some(Encoding::Gzip));
        assert_eq!(read("*;q=0, gzip").choose(all), Some(Encoding::Gzip));
        assert_eq!(read("identity, deflate").choose(all), None);
        assert_eq!(read("").choose(all), None);
    }

    #[test]
    fn refuses_a_weight_that_is_not_one() {
        for bad in ["gzip;q=2", "gzip;q=x", "gzip;q=1.5", "gzip;level=1", "g zip", "gzip;q=0.1234"] {
            assert!(Accepted::read(bad.as_bytes()).is_err(), "{bad}");
        }
    }
}
