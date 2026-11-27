// What a conversion across the plugin boundary refuses: a model value with no WIT case, or a
// WIT value the model does not accept. Part of `wit/mirror`, included by each side.

/// A model value with no case in this version of the WIT, named by its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unmirrored {
    /// A block kind.
    Block,
    /// An inline kind.
    Inline,
    /// A link target kind.
    Link,
    /// An image source kind.
    Image,
    /// A column alignment.
    Alignment,
    /// A callout kind.
    Callout,
    /// A page kind.
    Kind,
    /// A site's root.
    Root,
    /// A site's layout.
    Layout,
    /// A subject.
    Subject,
    /// A field type of the catalog.
    Field,
    /// A media type.
    Media,
    /// A highlighting capture.
    Capture,
    /// A document of more nodes than a `u32` counts.
    Large,
    /// A snapshot's version.
    Version,
    /// Where a fact came from.
    Provenance,
    /// How sure a fact is.
    Known,
    /// An ability of a source.
    Ability,
    /// How much of an ability was lacking.
    Gap,
    /// A refusal of the `git` capability.
    Fetch,
}

impl std::fmt::Display for Unmirrored {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "the plugin boundary has no case for this {self:?} of the model")
    }
}

impl std::error::Error for Unmirrored {}

/// A WIT value the model refuses: what it is, and why.
#[derive(Debug)]
#[non_exhaustive]
pub enum Malformed {
    /// A path that is no path in a snapshot.
    Path {
        /// As given.
        written: String,
        /// Why.
        source: model::PathError,
    },
    /// A route that is no route.
    Route {
        /// As given.
        written: String,
        /// Why.
        source: model::RouteError,
    },
    /// A slug that is no slug.
    Slug {
        /// As given.
        written: String,
        /// Why.
        source: model::SlugError,
    },
    /// A link out that is no URL the model accepts.
    Url {
        /// As given.
        written: String,
        /// Why.
        source: model::UrlError,
    },
    /// A date that is no day and no instant.
    When {
        /// As given.
        written: String,
        /// Why.
        source: model::WhenError,
    },
    /// A language that is no BCP 47 tag.
    Language {
        /// As given.
        written: String,
        /// Why.
        source: model::LanguageError,
    },
    /// A tag with no letter or digit.
    Tag {
        /// As given.
        written: String,
        /// Why.
        source: model::SlugError,
    },
    /// An author with an empty name.
    Author {
        /// Why.
        source: model::EmptyAuthor,
    },
    /// A decision code no decision has.
    Decision {
        /// Why.
        source: model::UnknownDecision,
    },
    /// A hash that is not 32 bytes.
    Hash {
        /// How many bytes it has.
        length: usize,
    },
    /// A heading level outside 1 to 6.
    Level {
        /// Why.
        source: model::LevelError,
    },
    /// A span that ends before it starts, or past what this machine counts.
    Span {
        /// Where it starts.
        start: u64,
        /// Where it ends.
        end: u64,
    },
    /// A problem or announcement the catalog does not have, or with the wrong fields.
    Coded {
        /// Why.
        source: model::coded::Refused,
    },
    /// A diagnostic with no problem in its chain.
    Chain,
    /// A child at or after its parent, or past the end of its list.
    Child {
        /// The child's place.
        at: u32,
    },
    /// A node two parents name.
    Shared {
        /// Its place.
        at: u32,
    },
    /// A node no parent names.
    Unused {
        /// Its place.
        at: usize,
    },
    /// Parts that make no site.
    Site {
        /// Why.
        source: model::SiteError,
    },
    /// Runs that do not cover the code, or split a character.
    Runs,
    /// A file the theme serves at a route that is no route.
    File {
        /// Why.
        source: Box<Malformed>,
    },
    /// A git object id or upload id that is no id.
    Version {
        /// Why.
        source: model::VersionError,
    },
    /// An instant outside what the model counts.
    Instant {
        /// Whole seconds since the epoch, as given.
        seconds: i64,
        /// The nanoseconds past them, as given.
        nanos: u32,
    },
    /// Entries and facts that make no snapshot.
    Snapshot {
        /// Why.
        source: model::SnapshotError,
    },
    /// An address that is no locator.
    Locator {
        /// Why.
        source: model::git::LocatorError,
    },
    /// A revision or credential name that is none.
    Name {
        /// Why.
        source: model::git::NameError,
    },
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path { written, .. } => write!(out, "`{written}` is no path in a snapshot"),
            Self::Route { written, .. } => write!(out, "`{written}` is no route"),
            Self::Slug { written, .. } => write!(out, "`{written}` is no slug"),
            Self::Url { written, .. } => write!(out, "`{written}` is no link out of the site"),
            Self::When { written, .. } => write!(out, "`{written}` is no date"),
            Self::Language { written, .. } => write!(out, "`{written}` is no language tag"),
            Self::Tag { written, .. } => write!(out, "`{written}` is no tag"),
            Self::Author { .. } => out.write_str("an author has no name"),
            Self::Decision { .. } => out.write_str("a decision code is unknown"),
            Self::Hash { length } => write!(out, "a hash has {length} bytes, not 32"),
            Self::Level { .. } => out.write_str("a heading level is outside 1 to 6"),
            Self::Span { start, end } => write!(out, "the span {start}..{end} is no span"),
            Self::Coded { .. } => out.write_str("a problem or announcement is not the catalog's"),
            Self::Chain => out.write_str("a diagnostic holds no problem"),
            Self::Child { at } => write!(out, "node {at} is named before it exists"),
            Self::Shared { at } => write!(out, "node {at} has two parents"),
            Self::Unused { at } => write!(out, "node {at} has no parent"),
            Self::Site { .. } => out.write_str("the parts make no site"),
            Self::Runs => out.write_str("the runs do not cover the code"),
            Self::File { .. } => out.write_str("a theme's file is not one the model accepts"),
            Self::Version { .. } => out.write_str("a version holds an id that is no id"),
            Self::Instant { seconds, nanos } => write!(out, "{seconds} s and {nanos} ns is no instant"),
            Self::Snapshot { .. } => out.write_str("the entries and facts make no snapshot"),
            Self::Locator { .. } => out.write_str("the address is no repository locator"),
            Self::Name { .. } => out.write_str("a revision or credential name is not one the model accepts"),
        }
    }
}

impl std::error::Error for Malformed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path { source, .. } => Some(source),
            Self::Route { source, .. } => Some(source),
            Self::Slug { source, .. } | Self::Tag { source, .. } => Some(source),
            Self::Url { source, .. } => Some(source),
            Self::When { source, .. } => Some(source),
            Self::Language { source, .. } => Some(source),
            Self::Author { source } => Some(source),
            Self::Decision { source } => Some(source),
            Self::Level { source } => Some(source),
            Self::Coded { source } => Some(source),
            Self::Site { source } => Some(source),
            Self::File { source } => Some(source.as_ref()),
            Self::Version { source } => Some(source),
            Self::Snapshot { source } => Some(source),
            Self::Locator { source } => Some(source),
            Self::Name { source } => Some(source),
            Self::Instant { .. }
            | Self::Hash { .. }
            | Self::Span { .. }
            | Self::Chain
            | Self::Child { .. }
            | Self::Shared { .. }
            | Self::Unused { .. }
            | Self::Runs => None,
        }
    }
}
