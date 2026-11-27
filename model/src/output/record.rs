//! What a build records about itself, so the next build can say what changed and why (decision
//! 0042): the inputs it was made from, every decision, every file of the snapshot and where
//! each published file is served. Locally it is kept beside the manifest in the output folder;
//! hosted, beside each deploy.
//!
//! [`Changes`] is what two builds' records differ by: newly public files first, then routes withdrawn
//! and moved, then each changed decision with its typed [`Cause`].

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Ability, AnyCode, AnyDecision, ContentPath, DeployId, Hash, Route, Snapshot, Subject, Version};

/// The record of one build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Build {
    /// What the build was made from.
    pub inputs: Inputs,
    /// Every decision, in the order made.
    pub decisions: Vec<AnyDecision>,
    /// Every file of the snapshot with its blob, in path order.
    pub files: Vec<Kept>,
    /// Every published file and where it is served, in path order.
    pub routes: Vec<Routed>,
}

impl Build {
    /// A record of `decisions`, `files` and `routes` made from `inputs`; files and routes are
    /// put in path order.
    pub fn new(inputs: Inputs, decisions: Vec<AnyDecision>, mut files: Vec<Kept>, mut routes: Vec<Routed>) -> Self {
        files.sort();
        routes.sort();
        Self { inputs, decisions, files, routes }
    }
}

/// The inputs of a build, as far as a change's cause needs them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Inputs {
    /// The version of the source read.
    pub source: Version,
    /// The release of ink that built it: informative, since the rules decide.
    pub release: Release,
    /// Every plugin that ran as a component, in id order.
    pub plugins: Vec<Plugin>,
    /// The blob of `ink.toml`, when there is one.
    pub config: Option<Hash>,
    /// What the source could do, in order.
    pub abilities: Vec<Ability>,
    /// What it could do only in part or not at all, such as history before a shallow clone's
    /// boundary, in order and once each.
    pub lacking: Vec<Ability>,
}

impl Inputs {
    /// The inputs of a build of `snapshot` by `release` with `plugins`, its `ink.toml` the blob
    /// `config`; plugins and abilities are put in order.
    pub fn of(snapshot: &Snapshot, release: Release, mut plugins: Vec<Plugin>, config: Option<Hash>) -> Self {
        plugins.sort();
        let observed = snapshot.observed();
        let mut abilities = observed.has.clone();
        abilities.sort();
        let mut lacking: Vec<Ability> = observed.lacks.iter().map(|lack| lack.ability).collect();
        lacking.sort();
        lacking.dedup();
        Self { source: snapshot.version().clone(), release, plugins, config, abilities, lacking }
    }
}

/// A release of ink, as its version is written: `0.3.1`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Release(String);

impl From<&str> for Release {
    fn from(version: &str) -> Self {
        Self(version.to_owned())
    }
}

impl fmt::Display for Release {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// One plugin that ran in a build as a component. Plugins compiled into ink are not listed:
/// they change only with its release.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Plugin {
    /// Its id.
    pub id: PluginId,
    /// The BLAKE3 digest of the component's bytes.
    pub digest: Hash,
}

impl Plugin {
    /// Plugin `id`, the component of `digest`.
    pub fn new(id: PluginId, digest: Hash) -> Self {
        Self { id, digest }
    }
}

/// A plugin's id, `<scope>:<slot>-<variant>`: `ink:format-markdown` (conventions.md, Grammar).
/// The scope and the name after `:` are lower-case letters, digits and single `-`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct PluginId(String);

/// Why text is no [`PluginId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginIdError {
    /// No `:` between the scope and the name.
    NoScope,
    /// The scope or the name is not kebab-case, or the name has no `<slot>-<variant>`.
    NotKebab,
}

impl fmt::Display for PluginIdError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoScope => out.write_str("a plugin id is `<scope>:<slot>-<variant>`"),
            Self::NotKebab => out.write_str("a plugin id is lower-case letters, digits and single `-`"),
        }
    }
}

impl std::error::Error for PluginIdError {}

impl FromStr for PluginId {
    type Err = PluginIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (scope, name) = text.split_once(':').ok_or(PluginIdError::NoScope)?;
        let kebab = |part: &str| {
            !part.is_empty()
                && part
                    .split('-')
                    .all(|word| !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
        };
        if kebab(scope) && kebab(name) && name.contains('-') {
            Ok(Self(text.to_owned()))
        } else {
            Err(PluginIdError::NotKebab)
        }
    }
}

impl PluginId {
    /// `ink:format-markdown`, the format bundled with ink for markdown.
    pub fn markdown() -> Self {
        Self("ink:format-markdown".to_owned())
    }

    /// `ink:format-obsidian`, the format bundled with ink for an Obsidian vault.
    pub fn obsidian() -> Self {
        Self("ink:format-obsidian".to_owned())
    }

    /// Whether it is a format, which reads files into pages: its name starts with `format-`.
    pub fn is_format(&self) -> bool {
        self.0.split_once(':').is_some_and(|(_, name)| name.starts_with("format-"))
    }
}

impl TryFrom<String> for PluginId {
    type Error = PluginIdError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<PluginId> for String {
    fn from(id: PluginId) -> Self {
        id.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// A file of a snapshot and its blob.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Kept {
    /// The file.
    pub path: ContentPath,
    /// Its bytes' hash.
    pub blob: Hash,
}

impl Kept {
    /// `path`, holding `blob`.
    pub fn new(path: ContentPath, blob: Hash) -> Self {
        Self { path, blob }
    }
}

/// A published file and its route.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Routed {
    /// The file.
    pub path: ContentPath,
    /// Where it is served.
    pub route: Route,
}

impl Routed {
    /// `path`, served at `route`.
    pub fn new(path: ContentPath, route: Route) -> Self {
        Self { path, route }
    }
}

/// What a change set compares with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Against {
    /// The previous build's record.
    Deploy {
        /// The previous deploy.
        deploy: DeployId,
    },
    /// A previous build that kept no record this ink reads, such as one an older ink wrote.
    Unrecorded {
        /// The previous deploy, when its manifest names it.
        deploy: Option<DeployId>,
    },
    /// No previous build: every published file is newly public.
    Nothing,
}

/// What a build changed against its baseline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Changes {
    /// What it compares with.
    pub against: Against,
    /// Files published now and not before, with their routes.
    pub public: Vec<Routed>,
    /// Files published before and not now, with the routes no longer served.
    pub withdrawn: Vec<Routed>,
    /// Files published at another route than before.
    pub moved: Vec<Moved>,
    /// Every decision whose value changed, was made for the first time, or is no longer made.
    pub decisions: Vec<Change>,
}

impl Changes {
    /// A change set against `against` with nothing changed yet.
    pub fn new(against: Against) -> Self {
        Self { against, public: Vec::new(), withdrawn: Vec::new(), moved: Vec::new(), decisions: Vec::new() }
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.public.is_empty() && self.withdrawn.is_empty() && self.moved.is_empty() && self.decisions.is_empty()
    }
}

/// A file served at a new route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Moved {
    /// The file.
    pub path: ContentPath,
    /// Where it was served.
    pub before: Route,
    /// Where it is served now.
    pub after: Route,
}

impl Moved {
    /// `path`, moved from `before` to `after`.
    pub fn new(path: ContentPath, before: Route, after: Route) -> Self {
        Self { path, before, after }
    }
}

/// One decision that changed, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Change {
    /// Its code.
    pub code: AnyCode,
    /// What it is about.
    pub subject: Subject,
    /// The decision before, if it was made.
    pub before: Option<AnyDecision>,
    /// The decision now, if it is made.
    pub after: Option<AnyDecision>,
    /// Why it changed.
    pub cause: Cause,
}

impl Change {
    /// `code` on `subject` went from `before` to `after` because of `cause`.
    pub fn new(
        (code, subject): (AnyCode, Subject),
        before: Option<AnyDecision>,
        after: Option<AnyDecision>,
        cause: Cause,
    ) -> Self {
        Self { code, subject, before, after, cause }
    }
}

/// Why a decision changed between two builds. The first that holds is the cause.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Cause {
    /// The content changed.
    Content(Touched),
    /// `ink.toml` changed: a pin or pattern decides it on either side, or the file differs.
    Config,
    /// The inference rules' version differs. Not given before `site.rules` exists (decision
    /// 0042, its second step).
    Rules,
    /// A format plugin that reads the file is another component, or ran in only one build.
    Plugin {
        /// The plugin.
        plugin: PluginId,
    },
    /// The source gained or lost an ability the decision reads, such as history, in whole or
    /// in part.
    Ability {
        /// The ability.
        ability: Ability,
    },
    /// Same content, config, plugins and abilities, another value: a change in ink itself.
    Engine,
}

/// What content changed for a decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Touched {
    /// The subject's file is new.
    Added,
    /// The subject's file is gone.
    Removed,
    /// The subject's file holds other bytes.
    Edited,
    /// Another file of the snapshot was added, removed or edited.
    Elsewhere,
}

impl fmt::Display for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content(Touched::Added) => out.write_str("content: the file is new"),
            Self::Content(Touched::Removed) => out.write_str("content: the file is gone"),
            Self::Content(Touched::Edited) => out.write_str("content: the file was edited"),
            Self::Content(Touched::Elsewhere) => out.write_str("content: another file changed"),
            Self::Config => out.write_str("config: ink.toml changed"),
            Self::Rules => out.write_str("rules: another rules version"),
            Self::Plugin { plugin } => write!(out, "plugin: another build of {plugin}"),
            Self::Ability { ability } => write!(out, "source: {} is gained or lost", Named(*ability)),
            Self::Engine => out.write_str("ink: nothing in the site changed"),
        }
    }
}

/// An ability, by its name.
#[derive(Clone, Copy, Debug)]
struct Named(Ability);

impl fmt::Display for Named {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Ability::Snapshot => "snapshot",
            Ability::History => "history",
            Ability::Watch => "watch",
            Ability::Private => "private",
        })
    }
}
