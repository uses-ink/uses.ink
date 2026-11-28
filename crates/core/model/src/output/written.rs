//! The file `ink build` writes into its output folder, and the one fact every reader and writer
//! of it shares: its name, its schema and how it starts.

use serde::{Deserialize, Serialize};

use super::manifest::DeployId;

/// The manifest as written into the output folder: compact, its schema first, so it starts with
/// [`Written::MARK`].
///
/// What the parts are is the caller's: `ink build` writes borrowed files, manifest and record, an
/// edge reads owned ones and ignores the record, so each is a parameter. The constants are named
/// without parameters, as `<Written>::MANIFEST`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Written<Files = (), Body = (), Record = ()> {
    /// The schema of what is written, [`Written::SCHEMA`] when this ink wrote it.
    pub schema: String,
    /// The deploy the manifest gives.
    pub deploy: DeployId,
    /// Every file written, so the next build can remove exactly those.
    pub files: Files,
    /// The manifest's entries.
    pub manifest: Body,
    /// What the build decided and was made from, so the next build can say what changed. A reader
    /// that ignores it takes a file that has none.
    #[serde(default)]
    pub record: Record,
}

impl Written {
    /// The manifest's file in the output folder. A dotted name is never a route, so no entry can
    /// take it, and the folder walk knows it by this name.
    pub const MANIFEST: &'static str = ".ink-manifest.json";

    /// The schema of what is written and read.
    pub const SCHEMA: &'static str = "uses.manifest/v3";

    /// How the file starts, any version: its schema, first.
    pub const MARK: &'static str = "{\"schema\":\"uses.manifest/";
}

impl<Files, Body, Record> Written<Files, Body, Record> {
    /// A file of this ink's own schema.
    #[must_use]
    pub fn new(deploy: DeployId, files: Files, manifest: Body, record: Record) -> Self {
        Self { schema: <Written>::SCHEMA.to_owned(), deploy, files, manifest, record }
    }
}
