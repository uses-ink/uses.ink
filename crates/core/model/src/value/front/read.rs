//! How a value type reads a frontmatter value.
use crate::{
    Author, ContentPath, Expected, Language, LanguageError, PageKind, Raw, Refusal, Slug, Tag, Value, Wanted, When,
    WhenError,
};

/// A value type a frontmatter key can hold. Which keys hold it is the code's business
/// ([`Keyed`](crate::code::Keyed)): `String` is a title under `title` and a summary under
/// `description`.
pub trait FromFront: Value {
    /// What it accepts, for the refusal.
    const WANTED: Wanted;

    /// The value `raw` holds, or why it holds none.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal>;

    /// The values `raw` holds when a list of them is wanted (`Vec<Self>`): each item of a list,
    /// or one value for a scalar. A list is refused whole when any item is, with every refused
    /// item's span.
    fn many(raw: &Raw<'_>) -> Result<Vec<Self>, Refusal> {
        match raw {
            Raw::List { items, .. } => Refusal::gather(items.iter().map(Self::read)),
            Raw::Map { span } => Err(Expected::new(Self::WANTED, *span).listed().into()),
            scalar => Self::read(scalar).map(|value| vec![value]),
        }
    }
}

impl<T: FromFront> FromFront for Option<T> {
    const WANTED: Wanted = T::WANTED;

    /// `None` for a null (`key:`) and for `false`, which says "none" in frontmatter as in
    /// `ink.toml` (`date: false` is undated); else the value of `T`. An `Option<bool>` therefore
    /// cannot hold `Some(false)`, and no code decides one.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        match raw {
            Raw::Null { .. } | Raw::Bool { value: false, .. } => Ok(None),
            _ => T::read(raw).map(Some),
        }
    }
}

impl<T: FromFront> FromFront for Vec<T> {
    const WANTED: Wanted = T::WANTED;

    /// Empty for a null (`tags:`); else [`FromFront::many`] of `T`.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        match raw {
            Raw::Null { .. } => Ok(Vec::new()),
            _ => T::many(raw),
        }
    }
}

impl FromFront for String {
    const WANTED: Wanted = Wanted::Text;

    /// The text, trimmed, with at least one character that is not a space.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        if text.is_empty() { Err(Expected::new(Self::WANTED, raw.span()).into()) } else { Ok(text.to_owned()) }
    }
}

impl FromFront for bool {
    const WANTED: Wanted = Wanted::Flag;

    /// Only a YAML boolean: the text `"true"` is refused, so a quoted value is not a flag.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        match raw {
            Raw::Bool { value, .. } => Ok(*value),
            _ => Err(Expected::new(Self::WANTED, raw.span()).into()),
        }
    }
}

impl FromFront for When {
    const WANTED: Wanted = Wanted::When;

    /// See [`When`]'s `FromStr`.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        text.parse().map_err(|cause: WhenError| Expected::new(Self::WANTED, raw.span()).because(cause).into())
    }
}

impl FromFront for Slug {
    const WANTED: Wanted = Wanted::Slug;

    /// Text that already is a slug: `hello-world`, not `Hello World`.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        Self::parse(text).map_err(|cause| Expected::new(Self::WANTED, raw.span()).because(cause).into())
    }
}

impl FromFront for Language {
    const WANTED: Wanted = Wanted::Language;

    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        text.parse().map_err(|cause: LanguageError| Expected::new(Self::WANTED, raw.span()).because(cause).into())
    }
}

impl FromFront for Author {
    const WANTED: Wanted = Wanted::Author;

    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        Self::try_from(text.to_owned()).map_err(|cause| Expected::new(Self::WANTED, raw.span()).because(cause).into())
    }
}

impl FromFront for ContentPath {
    const WANTED: Wanted = Wanted::Path;

    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        Self::parse(text).map_err(|cause| Expected::new(Self::WANTED, raw.span()).because(cause).into())
    }
}

impl FromFront for PageKind {
    const WANTED: Wanted = Wanted::Kind;

    /// `post` or `page`, in any case. The home page is decided by the site, not by its own
    /// frontmatter.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        if text.eq_ignore_ascii_case("post") {
            Ok(Self::Post)
        } else if text.eq_ignore_ascii_case("page") {
            Ok(Self::Page)
        } else {
            Err(Expected::new(Self::WANTED, raw.span()).into())
        }
    }
}

impl FromFront for Tag {
    const WANTED: Wanted = Wanted::Tag;

    /// The text, trimmed, with at least one letter or digit.
    fn read(raw: &Raw<'_>) -> Result<Self, Refusal> {
        let text = raw.scalar(Self::WANTED)?;
        Self::new(text).map_err(|cause| Expected::new(Self::WANTED, raw.span()).because(cause).into())
    }

    /// A list of tags, or one text of tags separated by commas (`tags: rust, notes`). Every
    /// refused tag of a text has the text's span.
    fn many(raw: &Raw<'_>) -> Result<Vec<Self>, Refusal> {
        match raw {
            Raw::List { items, .. } => Refusal::gather(items.iter().map(Self::read)),
            Raw::Map { span } => Err(Expected::new(Self::WANTED, *span).listed().into()),
            scalar => {
                let text = scalar.scalar(Self::WANTED)?;
                let span = scalar.span();
                let written = text.split(',').map(str::trim).filter(|tag| !tag.is_empty());
                Refusal::gather(
                    written.map(|tag| {
                        Self::new(tag).map_err(|cause| Expected::new(Self::WANTED, span).because(cause).into())
                    }),
                )
            }
        }
    }
}
