//! The three byte strings of sealing.

use std::fmt;

use zeroize::Zeroizing;

/// A credential in the clear. It prints as `Plain(<redacted>)`, is zeroed when dropped, and
/// cannot be cloned, displayed or serialized; the only way to its bytes is [`Plain::expose`].
pub struct Plain(Zeroizing<Vec<u8>>);

impl Plain {
    /// The credential's bytes. Naming the call makes every read of a secret easy to find.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Plain {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Plain(<redacted>)")
    }
}

impl From<Vec<u8>> for Plain {
    fn from(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

impl From<&str> for Plain {
    fn from(text: &str) -> Self {
        Self(Zeroizing::new(text.as_bytes().to_vec()))
    }
}

/// A credential sealed: what is stored in the row. The format is the adapter's own; to the rest
/// of the code it is opaque bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Sealed(Vec<u8>);

impl fmt::Debug for Sealed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Sealed({} bytes)", self.0.len())
    }
}

impl AsRef<[u8]> for Sealed {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for Sealed {
    /// Any bytes: whether they open is [`crate::Secret::open`]'s to say.
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

/// What a sealed value is bound to: the id of the row that holds it. It is authenticated, not
/// secret, and stored nowhere with the value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding(Vec<u8>);

impl AsRef<[u8]> for Binding {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<&str> for Binding {
    fn from(row: &str) -> Self {
        Self(row.as_bytes().to_vec())
    }
}

impl From<Vec<u8>> for Binding {
    fn from(row: Vec<u8>) -> Self {
        Self(row)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_secret_prints_nothing() {
        assert_eq!(format!("{:?}", Plain::from("hunter2")), "Plain(<redacted>)");
        assert!(!format!("{:?}", Sealed::from(b"hunter2".to_vec())).contains("hunter2"));
    }
}
