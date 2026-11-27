// Text and bytes from the WIT, read as the model's validated types. Part of `wit/mirror`.

use model::{Anchor, AnyCode, ContentPath, ExternalUrl, FootnoteName, Hash, Language, Oid, Route, Slug, Tag, When};

use super::refusal::Malformed;

/// Text from the WIT, read as a model type by its `FromStr` or `TryFrom<String>`.
pub(crate) struct Written(pub(crate) String);

impl TryFrom<Written> for ContentPath {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        Self::parse(written.clone()).map_err(|source| Malformed::Path { written, source })
    }
}

impl TryFrom<Written> for Route {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        Self::parse(written.clone()).map_err(|source| Malformed::Route { written, source })
    }
}

impl TryFrom<Written> for Slug {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        Self::parse(written.clone()).map_err(|source| Malformed::Slug { written, source })
    }
}

impl TryFrom<Written> for ExternalUrl {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        Self::parse(written.clone()).map_err(|source| Malformed::Url { written, source })
    }
}

impl TryFrom<Written> for When {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        written.parse().map_err(|source| Malformed::When { written, source })
    }
}

impl TryFrom<Written> for Language {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        written.parse().map_err(|source| Malformed::Language { written, source })
    }
}

impl TryFrom<Written> for AnyCode {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        written.parse().map_err(|source| Malformed::Decision { source })
    }
}

impl TryFrom<Written> for Oid {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        Self::parse(written).map_err(|source| Malformed::Version { source })
    }
}

impl TryFrom<Written> for Anchor {
    type Error = Malformed;

    fn try_from(written: Written) -> Result<Self, Malformed> {
        Slug::try_from(written).map(Self::new)
    }
}

impl TryFrom<Written> for FootnoteName {
    type Error = Malformed;

    fn try_from(written: Written) -> Result<Self, Malformed> {
        Slug::try_from(written).map(Self::from)
    }
}

impl TryFrom<Written> for Tag {
    type Error = Malformed;

    fn try_from(Written(written): Written) -> Result<Self, Malformed> {
        Self::new(written.clone()).map_err(|source| Malformed::Tag { written, source })
    }
}

/// The 32 bytes of a hash, from the WIT.
pub(crate) struct Bytes(pub(crate) Vec<u8>);

impl TryFrom<Bytes> for Hash {
    type Error = Malformed;

    fn try_from(Bytes(bytes): Bytes) -> Result<Self, Malformed> {
        let length = bytes.len();
        <[u8; 32]>::try_from(bytes).map(Self::from_bytes).map_err(|_| Malformed::Hash { length })
    }
}

/// Reads `text` from the WIT as `T`.
impl Written {
    pub(crate) fn read<T: TryFrom<Self, Error = Malformed>>(text: String) -> Result<T, Malformed> {
        T::try_from(Self(text))
    }

    pub(crate) fn maybe<T: TryFrom<Self, Error = Malformed>>(text: Option<String>) -> Result<Option<T>, Malformed> {
        text.map(Self::read).transpose()
    }
}
