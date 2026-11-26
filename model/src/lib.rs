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
//! - Output: [`Manifest`], whose canonical bytes give the [`DeployId`].
//! - Problems: [`Diagnostic`] and [`Notice`], over [`Problem`] and [`Announcement`], which
//!   `cargo xtask diag` generates from `diag/catalog.toml`.
//!
//! Nothing here reads a clock, the environment or a file: these are values.

pub mod code;
pub mod decision;
mod diagnostic;
mod document;
mod format;
#[allow(clippy::too_many_lines, clippy::match_same_arms, reason = "generated: one match arm per catalog entry")]
mod generated;
mod hash;
pub mod highlight;
mod language;
mod manifest;
mod path;
mod route;
mod site;
mod size;
mod snapshot;
mod span;
mod subject;
mod time;
mod url;
mod value;

pub use code::{AnyCode, AnyDecision, AnyOf, Code, Each, Family, Keyed, Typed, UnknownDecision, Word, Worded};
pub use decision::{Alternative, Decision, DefaultReason, Evidence, Rejection};
pub use diagnostic::{Diagnostic, Fault, Notice, Severity};
pub use document::{
    Alignment, Anchor, Block, CalloutKind, Definition, Document, Fold, Footnote, FootnoteName, ImageSource, Inline,
    Item, Level, LevelError, LinkTarget, Task, Visit,
};
pub use format::{Front, Read, Spanned};
pub use generated::{Announcement, Problem};
pub use hash::{Hash, HashError};
pub use language::{Language, LanguageError};
pub use manifest::{
    DeployId, DuplicateRoute, Header, HeaderName, HeaderValue, HeaderValueError, Manifest, MediaType, Served, Status,
};
pub use path::{ContentPath, PathError};
pub use route::{Route, RouteError, Slug, SlugError};
pub use site::{
    Asset, Author, EmptyAuthor, Excluded, Identity, Layout, Page, PageId, PageKind, Root, Site, SiteError, SiteParts,
    Tag, TagError,
};
pub use size::Bytes;
pub use snapshot::{
    Ability, Entry, FileFacts, Known, Lack, Observed, Oid, Provenance, Snapshot, SnapshotError, UnknownReason,
    UploadId, Version, VersionError,
};
pub use span::{Backwards, Span};
pub use subject::Subject;
pub use time::{Timestamp, When, WhenError};
pub use url::{ExternalUrl, UrlError};
pub use value::{Cause, Expected, Form, FromFront, Raw, Refusal, Shown, Value, Wanted};
