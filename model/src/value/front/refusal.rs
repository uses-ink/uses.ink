//! What a refused value wanted, and the refusal a frontmatter read collects.

use crate::{EmptyAuthor, LanguageError, PathError, SlugError, Span, WhenError};
use std::fmt;

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
