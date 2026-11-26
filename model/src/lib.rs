//! The types every part of uses.ink speaks.
//!
//! - Content: [`Hash`], [`ContentPath`], [`Snapshot`] and the facts a source knows.
//! - Interpretation: [`Decision`] and [`Decided`], one variant per decision code.
//! - The result: [`Site`], [`Page`], [`Document`].
//! - Output: [`Manifest`], whose canonical bytes give the [`DeployId`].
//! - Problems: [`Diagnostic`] and [`Notice`], over [`Problem`] and [`Announcement`], which
//!   `cargo xtask diag` generates from `diag/catalog.toml`.
//!
//! Nothing here reads a clock, the environment or a file: these are values.

mod decision;
mod diagnostic;
mod document;
mod format;
#[allow(clippy::too_many_lines, clippy::match_same_arms, reason = "generated: one match arm per catalog entry")]
mod generated;
mod hash;
mod language;
mod manifest;
mod path;
mod route;
mod site;
mod size;
mod snapshot;
mod span;
mod time;
mod url;

pub use decision::{
    Alternative, Decided, Decision, DecisionCode, DefaultReason, Evidence, Rejection, Root, Subject, UnknownDecision,
};
pub use diagnostic::{Diagnostic, Fault, Notice, Severity};
pub use document::{
    Alignment, Anchor, Block, CalloutKind, Definition, Document, Footnote, FootnoteName, ImageSource, Inline, Item,
    Level, LevelError, LinkTarget, Task,
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
    Asset, Author, EmptyAuthor, Excluded, Identity, Layout, Page, PageId, PageKind, Site, SiteError, SiteParts, Tag,
};
pub use size::Bytes;
pub use snapshot::{
    Ability, Entry, FileFacts, Known, Lack, Observed, Oid, Provenance, Snapshot, SnapshotError, UnknownReason,
    UploadId, Version, VersionError,
};
pub use span::{Backwards, Span};
pub use time::{Timestamp, When};
pub use url::{ExternalUrl, UrlError};
