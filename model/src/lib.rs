//! The types every part of uses.ink speaks.
//!
//! - Content: [`Hash`], [`ContentPath`], [`Snapshot`] and the facts a source knows.
//! - Interpretation: typed decision codes ([`code`]) and their [`Decision`], erased as
//!   [`AnyDecision`] for storage and the wire, each value worded by its code ([`Word`]).
//! - Values: [`Value`], and [`FromFront`] for what frontmatter can hold.
//! - The result: [`Site`], [`Page`], [`Document`], read with [`Visit`] and rewritten with
//!   [`Fold`].
//! - Highlighting: the [`highlight::Highlight`] capability a theme asks for a code block's
//!   classed runs.
//! - Images: the [`Picture`] the engine prepares from a raster asset, for a theme to show.
//! - Output: [`Manifest`], whose canonical bytes give the [`DeployId`], and the
//!   [`record`] a build keeps of itself, two of which give the changes between them.
//! - Problems: [`Diagnostic`] and [`Notice`], over [`Problem`] and [`Announcement`], which
//!   `cargo xtask diag` generates from `diag/catalog.toml`, with their [`coded`] form for the
//!   plugin boundary, and the [`Record`] a plugin says them into.
//!
//! Nothing here reads a clock, the environment or a file: these are values. The crate is
//! MIT OR Apache-2.0, unlike the engine, because every plugin speaks it (decision 0026).

mod chain;
mod content;
pub mod decision;
#[allow(clippy::too_many_lines, clippy::match_same_arms, reason = "generated: one match arm per catalog entry")]
mod generated;
mod output;
mod site;
mod value;

pub use decision::{code, coded};
pub use output::written::Written;
pub use output::{git, record};
pub use site::highlight;

pub use chain::Chain;
pub use content::hash::{Hash, HashError};
pub use content::language::{Language, LanguageError};
pub use content::path::{ContentPath, PathError};
pub use content::size::Bytes;
pub use content::snapshot::{
    Ability, Entry, FileFacts, Gap, Known, Lack, Observed, Oid, Provenance, Snapshot, SnapshotError, UnknownReason,
    UploadId, Version, VersionError,
};
pub use content::span::{Backwards, Span};
pub use content::time::{Timestamp, When, WhenError};
pub use content::url::{ExternalUrl, UrlError};
pub use decision::code::{
    AnyCode, AnyDecision, AnyOf, Code, Each, Family, Keyed, Typed, UnknownDecision, Word, Worded,
};
pub use decision::subject::Subject;
pub use decision::{Basis, Decision, DefaultReason, Evidence};
pub use generated::{Announcement, Problem};
pub use output::diagnostic::{Diagnostic, Fault, Notice, Record, Said, Severity};
pub use output::format::{Front, Read, Spanned};
pub use output::manifest::{
    Canonical, DeployId, DuplicateRoute, File, Header, HeaderName, HeaderValue, HeaderValueError, Manifest, MediaType,
    MediaUnknown, Served, Status,
};
pub use site::document::{
    Alignment, Anchor, Block, CalloutKind, Definition, Document, Fold, Folding, Footnote, FootnoteName, ImageSource,
    Inline, Item, Level, LevelError, LinkTarget, Task, Visit,
};
pub use site::picture::{Dimensions, Picture, Variant};
pub use site::route::{Route, RouteError, Slug, SlugError};
pub use site::{
    Asset, Author, EmptyAuthor, Excluded, Identity, Layout, Page, PageId, PageKind, Root, Site, SiteError, SiteParts,
    Tag, TagError,
};
pub use value::{Cause, Expected, Form, FromFront, Raw, Refusal, Shown, Value, Wanted};
