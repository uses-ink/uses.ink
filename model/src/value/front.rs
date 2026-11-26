//! Frontmatter values: what a format hands over ([`Raw`]), how a value type reads it
//! ([`FromFront`]), and what a refused value wanted ([`Expected`]).
//!
//! The format parses its frontmatter syntax (YAML for markdown) into [`Raw`] and reports what
//! does not parse; the value types here say what they accept, once, for every format.

use std::borrow::Cow;
use std::fmt;

use crate::{
    Author, ContentPath, EmptyAuthor, Language, LanguageError, PageKind, PathError, Slug, SlugError, Span, Tag, When,
    WhenError,
};

use super::Value;

/// One frontmatter value as the author wrote it, before any value type reads it. Each part
/// keeps its span; a number or a boolean keeps its text as written, so a key that wants text
/// reads `title: 2024` as `2024` and `title: 1.10` as `1.10`, not as a re-printed number.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Raw<'a> {
    /// The key with no value: `key:`, `key: null`, `key: ~`.
    Null {
        /// Where the value is (or, for a bare `key:`, the key).
        span: Span,
    },
    /// `true` or `false`.
    Bool {
        /// The value.
        value: bool,
        /// The text as written.
        written: &'a str,
        /// Where it is.
        span: Span,
    },
    /// A number, kept as written.
    Number {
        /// The text as written.
        written: &'a str,
        /// Where it is.
        span: Span,
    },
    /// Text, its quotes and escapes resolved.
    Text {
        /// The text.
        value: Cow<'a, str>,
        /// Where it is, quotes included.
        span: Span,
    },
    /// A list, each item with its own span.
    List {
        /// The items, in order.
        items: Vec<Self>,
        /// From the list's start to the end of its last item.
        span: Span,
    },
    /// A mapping. No value a decision reads is one; it is kept to be refused with its span.
    Map {
        /// Where it is.
        span: Span,
    },
}

impl Raw<'_> {
    /// Where the value is.
    pub const fn span(&self) -> Span {
        match self {
            Self::Null { span }
            | Self::Bool { span, .. }
            | Self::Number { span, .. }
            | Self::Text { span, .. }
            | Self::List { span, .. }
            | Self::Map { span } => *span,
        }
    }

    /// The value as text: the text itself, or a number or boolean as written. `None` for a
    /// null, a list or a mapping.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { value, .. } => Some(value),
            Self::Bool { written, .. } | Self::Number { written, .. } => Some(written),
            Self::Null { .. } | Self::List { .. } | Self::Map { .. } => None,
        }
    }

    /// The value as text, trimmed, or the refusal of it as not `wanted`.
    pub fn scalar(&self, wanted: Wanted) -> Result<&str, Expected> {
        self.text().map(str::trim).ok_or_else(|| Expected::new(wanted, self.span()))
    }
}

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

/// What a frontmatter value had to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Wanted {
    /// Some text that is not only spaces.
    Text,
    /// `true` or `false`.
    Flag,
    /// A day or an instant.
    When,
    /// A slug.
    Slug,
    /// A tag.
    Tag,
    /// A page kind.
    Kind,
    /// A language tag.
    Language,
    /// An author's name.
    Author,
    /// A path in the snapshot.
    Path,
}

impl fmt::Display for Wanted {
    /// The `expected` of `frontmatter.value.invalid`: `some text`, `` `true` or `false` ``.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Text => "some text",
            Self::Flag => "`true` or `false`",
            Self::When => "a day (`2024-03-01`) or a time with its offset (`2024-03-01T09:30:00+01:00`)",
            Self::Slug => "a slug: lower-case letters and digits, joined by single `-`",
            Self::Tag => "a tag with a letter or digit",
            Self::Kind => "`post` or `page`",
            Self::Language => "a language tag such as `en` or `fr-CH`",
            Self::Author => "a name that is not only spaces",
            Self::Path => "a path in the snapshot such as `posts/hello.md`",
        })
    }
}

/// Whether one value or a list of them was wanted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Form {
    /// One value.
    One,
    /// A list of values.
    List,
}

/// A refused frontmatter value: what it had to be, where it is, and the value type's own
/// reason when it has one.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Expected {
    /// What the value had to be.
    pub wanted: Wanted,
    /// One value, or a list of them.
    pub form: Form,
    /// Where the refused value is.
    pub span: Span,
    /// Why the value type refused the text, when it says.
    pub cause: Option<Cause>,
}

impl Expected {
    /// One value that had to be `wanted`, at `span`, with no reason of its own.
    pub const fn new(wanted: Wanted, span: Span) -> Self {
        Self { wanted, form: Form::One, span, cause: None }
    }

    /// The same refusal, of a value that had to be a list.
    #[must_use]
    pub fn listed(self) -> Self {
        Self { form: Form::List, ..self }
    }

    /// The same refusal, with the value type's reason.
    #[must_use]
    pub fn because(self, cause: impl Into<Cause>) -> Self {
        Self { cause: Some(cause.into()), ..self }
    }
}

impl fmt::Display for Expected {
    /// What was wanted, as the `expected` of `frontmatter.value.invalid`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.form {
            Form::One => write!(out, "{}", self.wanted),
            Form::List => write!(out, "a list, each item {}", self.wanted),
        }
    }
}

impl std::error::Error for Expected {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_ref().map(|cause| cause as &(dyn std::error::Error + 'static))
    }
}

/// Why a value type refused the text it was given.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Cause {
    /// Not a slug, or no letter or digit to make one.
    Slug(SlugError),
    /// Not a language tag.
    Language(LanguageError),
    /// Not a path in the snapshot.
    Path(PathError),
    /// Not a day or an instant.
    When(WhenError),
    /// An empty name.
    Author(EmptyAuthor),
}

impl fmt::Display for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slug(error) => fmt::Display::fmt(error, out),
            Self::Language(error) => fmt::Display::fmt(error, out),
            Self::Path(error) => fmt::Display::fmt(error, out),
            Self::When(error) => fmt::Display::fmt(error, out),
            Self::Author(error) => fmt::Display::fmt(error, out),
        }
    }
}

impl std::error::Error for Cause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::When(error) => error.source(),
            Self::Slug(_) | Self::Language(_) | Self::Path(_) | Self::Author(_) => None,
        }
    }
}

impl From<SlugError> for Cause {
    fn from(error: SlugError) -> Self {
        Self::Slug(error)
    }
}

impl From<LanguageError> for Cause {
    fn from(error: LanguageError) -> Self {
        Self::Language(error)
    }
}

impl From<PathError> for Cause {
    fn from(error: PathError) -> Self {
        Self::Path(error)
    }
}

impl From<WhenError> for Cause {
    fn from(error: WhenError) -> Self {
        Self::When(error)
    }
}

impl From<EmptyAuthor> for Cause {
    fn from(error: EmptyAuthor) -> Self {
        Self::Author(error)
    }
}

/// Every refused part of one frontmatter value, never empty: one for a scalar, one per refused
/// item for a list. A format reports each as its own `frontmatter.value.invalid`.
#[derive(Clone, Debug)]
pub struct Refusal {
    first: Expected,
    rest: Vec<Expected>,
}

impl Refusal {
    /// The first refused part.
    pub const fn first(&self) -> &Expected {
        &self.first
    }

    /// Every refused part, in the order written.
    pub fn iter(&self) -> impl Iterator<Item = &Expected> {
        std::iter::once(&self.first).chain(&self.rest)
    }

    /// Every value of `results`, or every refusal among them.
    pub fn gather<T>(results: impl IntoIterator<Item = Result<T, Self>>) -> Result<Vec<T>, Self> {
        let mut values = Vec::new();
        let mut refused: Option<Self> = None;
        for result in results {
            match (result, &mut refused) {
                (Ok(value), _) => values.push(value),
                (Err(refusal), Some(all)) => all.extend(refusal),
                (Err(refusal), None) => refused = Some(refusal),
            }
        }
        refused.map_or(Ok(values), Err)
    }
}

impl From<Expected> for Refusal {
    fn from(expected: Expected) -> Self {
        Self { first: expected, rest: Vec::new() }
    }
}

impl Extend<Expected> for Refusal {
    fn extend<I: IntoIterator<Item = Expected>>(&mut self, refused: I) {
        self.rest.extend(refused);
    }
}

impl IntoIterator for Refusal {
    type Item = Expected;
    type IntoIter = std::iter::Chain<std::iter::Once<Expected>, std::vec::IntoIter<Expected>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.first).chain(self.rest)
    }
}

impl fmt::Display for Refusal {
    /// The first refused part's [`Expected`]; the others are reported on their own.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.first, out)
    }
}

impl std::error::Error for Refusal {
    /// The first refused part's cause: its text is already the refusal's.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.first)
    }
}
