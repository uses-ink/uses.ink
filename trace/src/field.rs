//! Typed field values.
//!
//! `tracing`'s `Value` trait is sealed, so a newtype cannot implement it. [`Field`] is the
//! trait this crate puts in its place: each type below says which primitive, or which lazy
//! `Display`, it records as. A call site names the type, never a format string:
//! `bytes = Size(n).field()`. The conversion is an argument of the macro, so it runs only when
//! the level is enabled.

use std::error::Error;
use std::fmt;
use std::path::{Display as PathDisplay, Path};
use std::time::Duration;

use tracing::field::{DisplayValue, display};

/// What a type records as. `Out` is a type `tracing` accepts.
pub trait Field {
    /// The recorded form.
    type Out: tracing::Value;

    /// Converts for recording. Called only when the callsite is enabled.
    fn field(self) -> Self::Out;
}

/// A closed set of names, recorded as the `kind` of a span or the `outcome` of an event.
pub trait Kind: Copy {
    /// The name, a constant of the type.
    fn kind(self) -> &'static str;
}

impl Kind for &'static str {
    fn kind(self) -> &'static str {
        self
    }
}

/// A number of bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size(pub u64);

/// A count of things.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count(pub u64);

/// An HTTP status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status(pub u16);

/// How long something took, recorded as whole microseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Took(pub Duration);

macro_rules! numbers {
    ($($name:ident: $from:ty => |$value:ident| $into:expr),+ $(,)?) => {$(
        impl Field for $name {
            type Out = u64;

            fn field(self) -> u64 {
                let $value = self.0;
                $into
            }
        }

        impl From<$from> for $name {
            fn from(value: $from) -> Self {
                Self(value)
            }
        }
    )+};
}

numbers! {
    Size: u64 => |n| n,
    Count: u64 => |n| n,
    Status: u16 => |n| u64::from(n),
    Took: Duration => |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX),
}

/// The first bytes of a digest, as hex: enough to tell two apart in a log, not enough to name
/// one in a URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digest<'a>(pub &'a [u8]);

/// The hex a [`Digest`] records as.
#[derive(Clone, Copy, Debug)]
pub struct Short<'a>(&'a [u8]);

impl Short<'_> {
    /// How many bytes of a digest are shown.
    pub const BYTES: usize = 6;
}

impl fmt::Display for Short<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().take(Self::BYTES).try_for_each(|byte| write!(out, "{byte:02x}"))
    }
}

impl<'a> Field for Digest<'a> {
    type Out = DisplayValue<Short<'a>>;

    fn field(self) -> Self::Out {
        display(Short(self.0))
    }
}

/// Anything with a `Display`, recorded lazily: a site id, a content path, a host name.
#[derive(Clone, Copy, Debug)]
pub struct Shown<T>(pub T);

impl<T: fmt::Display> Field for Shown<T> {
    type Out = DisplayValue<T>;

    fn field(self) -> Self::Out {
        display(self.0)
    }
}

/// A path, as given.
#[derive(Clone, Copy, Debug)]
pub struct Located<'a>(pub &'a Path);

impl<'a> Field for Located<'a> {
    type Out = DisplayValue<PathDisplay<'a>>;

    fn field(self) -> Self::Out {
        display(self.0.display())
    }
}

/// What a [`Secret`] prints as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Redacted;

impl Redacted {
    /// The text.
    pub const TEXT: &'static str = "[redacted]";
}

impl fmt::Display for Redacted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(Self::TEXT)
    }
}

/// A value that must never reach a log: a token, a key, a cookie, an `Authorization` value.
/// Its `Debug`, its `Display` and its recorded form are all [`Redacted::TEXT`], whatever it
/// holds, so a log line that names it is safe by construction.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Secret<T>(pub T);

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(Redacted::TEXT)
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(Redacted::TEXT)
    }
}

impl<T> Field for Secret<T> {
    type Out = DisplayValue<Redacted>;

    fn field(self) -> Self::Out {
        display(Redacted)
    }
}

/// An error, recorded with its whole `source()` chain: the layers walk it, so no call site
/// flattens a cause into a string.
#[derive(Clone, Copy)]
pub struct Failure<'a>(pub &'a (dyn Error + 'static));

impl fmt::Debug for Failure<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_tuple("Failure").field(&self.0.to_string()).finish()
    }
}

impl<'a> Field for Failure<'a> {
    type Out = &'a (dyn Error + 'static);

    fn field(self) -> Self::Out {
        self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_secret_prints_nothing_of_what_it_holds() {
        let secret = Secret("hunter2");
        assert_eq!(format!("{secret:?}"), Redacted::TEXT);
        assert_eq!(format!("{secret}"), Redacted::TEXT);
        assert_eq!(format!("{secret:#?}"), Redacted::TEXT);
        assert_eq!(format!("{:?}", Secret(vec![1_u8, 2, 3])), Redacted::TEXT);
        assert_eq!(format!("{}", secret.field()), Redacted::TEXT);
    }

    #[test]
    fn a_duration_records_as_microseconds_and_saturates() {
        assert_eq!(Took(Duration::from_millis(3)).field(), 3000);
        assert_eq!(Took(Duration::MAX).field(), u64::MAX);
    }

    #[test]
    fn a_digest_shows_six_bytes_as_hex() {
        assert_eq!(format!("{}", Digest(&[0xab; 32]).field()), "abababababab");
        assert_eq!(format!("{}", Digest(&[1, 2]).field()), "0102");
    }
}
