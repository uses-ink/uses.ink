//! The `[image]` table: what a site owner chooses about the copies made of its images.
//!
//! ```toml
//! [image]
//! widths = [480, 960, 1440]   # pixel widths of the copies, narrowest first
//! avif-quality = 70           # 1 to 100
//! jpeg-quality = 80           # 1 to 100
//! ```
//!
//! A key left out keeps the default, which is the value above. Every key is checked here against
//! the range the encoder works in, so a number outside it is a `config.value.invalid` naming
//! both bounds, and the encoder's own clamps never act on a site's value (decision 0060). The
//! speed of the AVIF encoder and the pixel budget are the operator's, not the site's, and are
//! not keys.

use std::fmt;

use model::{Span, Spanned};

/// The `[image]` table, each key with its span; a key that is not there is `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    /// `widths = [...]`.
    pub widths: Option<Spanned<Widths>>,
    /// `avif-quality = ...`.
    pub avif_quality: Option<Spanned<Quality>>,
    /// `jpeg-quality = ...`.
    pub jpeg_quality: Option<Spanned<Quality>>,
    /// The `[image]` table itself, `None` when there is none.
    pub span: Option<Span>,
}

/// An encoder quality, 1 to 100.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quality(u8);

/// A copy's width in pixels, 1 to 65535: the widest side a JPEG can have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Width(u16);

/// The widths of the copies: at least one, each wider than the one before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Widths(Vec<Width>);

/// Why a number or a list is not an image setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageError {
    /// A number outside the range its key takes.
    Outside {
        /// What was written.
        got: i64,
        /// The least it may be.
        low: u16,
        /// The most it may be.
        high: u16,
    },
    /// An empty list of widths: no copy would be made.
    NoWidths,
    /// A width that is not wider than the one before it.
    Unordered,
}

impl fmt::Display for ImageError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Outside { got, low, high } => write!(out, "{got} is not from {low} to {high}"),
            Self::NoWidths => out.write_str("a list of no widths makes no copy"),
            Self::Unordered => out.write_str("each width must be wider than the one before it"),
        }
    }
}

impl std::error::Error for ImageError {}

impl Quality {
    /// The least a quality may be.
    pub const LOW: u8 = 1;
    /// The most a quality may be.
    pub const HIGH: u8 = 100;

    /// The quality, 1 to 100.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<i64> for Quality {
    type Error = ImageError;

    fn try_from(got: i64) -> Result<Self, Self::Error> {
        match u8::try_from(got) {
            Ok(number @ Self::LOW..=Self::HIGH) => Ok(Self(number)),
            _ => Err(ImageError::Outside { got, low: Self::LOW.into(), high: Self::HIGH.into() }),
        }
    }
}

impl Width {
    /// The most a width may be.
    pub const HIGH: u16 = u16::MAX;

    /// The width in pixels.
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<i64> for Width {
    type Error = ImageError;

    fn try_from(got: i64) -> Result<Self, Self::Error> {
        match u16::try_from(got) {
            Ok(number @ 1..) => Ok(Self(number)),
            _ => Err(ImageError::Outside { got, low: 1, high: Self::HIGH }),
        }
    }
}

impl TryFrom<Vec<Width>> for Widths {
    type Error = ImageError;

    fn try_from(widths: Vec<Width>) -> Result<Self, Self::Error> {
        if widths.is_empty() {
            return Err(ImageError::NoWidths);
        }
        if widths.iter().zip(widths.iter().skip(1)).any(|(before, after)| before >= after) {
            return Err(ImageError::Unordered);
        }
        Ok(Self(widths))
    }
}

impl Widths {
    /// The widths, narrowest first.
    pub fn get(&self) -> &[Width] {
        &self.0
    }
}
