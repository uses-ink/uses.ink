//! Text between tags and in attribute values, as a browser shows it.

use std::fmt::{self, Write as _};

/// Text from HTML with the common character references decoded: the five XML ones, `&nbsp;`,
/// and numeric ones. Any other `&` stays as written.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Decoded<'a>(pub(crate) &'a str);

impl fmt::Display for Decoded<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rest = self.0;
        while let Some(amp) = rest.find('&') {
            out.write_str(rest.get(..amp).unwrap_or_default())?;
            let after = rest.get(amp..).unwrap_or_default();
            let reference = Reference::at(after).unwrap_or(Reference { decoded: '&', len: 1 });
            out.write_char(reference.decoded)?;
            rest = after.get(reference.len..).unwrap_or_default();
        }
        out.write_str(rest)
    }
}

/// One character reference: the character it stands for and its length as written.
#[derive(Clone, Copy, Debug)]
struct Reference {
    decoded: char,
    len: usize,
}

impl Reference {
    /// The reference at the start of `text`, which holds `&`, if it is one this decodes.
    fn at(text: &str) -> Option<Self> {
        let end = text.get(..12).unwrap_or(text).find(';')?;
        let body = text.get(1..end)?;
        let decoded = match body {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "nbsp" => '\u{a0}',
            _ => {
                let number = body.strip_prefix('#')?;
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => number.parse().ok()?,
                };
                char::from_u32(code).filter(|&c| c != '\0')?
            }
        };
        Some(Self { decoded, len: end + 1 })
    }
}

/// Text with each run of HTML whitespace shown as one space, as a browser shows it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Collapsed<'a>(pub(crate) &'a str);

impl fmt::Display for Collapsed<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut space = false;
        for c in self.0.chars() {
            if c.is_ascii_whitespace() {
                space = true;
            } else {
                if space {
                    out.write_char(' ')?;
                    space = false;
                }
                out.write_char(c)?;
            }
        }
        if space {
            out.write_char(' ')?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{Collapsed, Decoded};

    #[test]
    fn decodes_common_references() {
        assert_eq!(Decoded("a &amp; b &lt;&#x41;&#66;&bogus; &").to_string(), "a & b <AB&bogus; &");
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(Collapsed(" a \n\t b ").to_string(), " a b ");
    }
}
