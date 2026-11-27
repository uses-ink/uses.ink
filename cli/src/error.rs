//! What stops a command before any diagnostic could: the folder or repository refused, an output
//! folder `ink` will not write into, a terminal that cannot be written to.

use std::fmt;
use std::path::PathBuf;

use crate::exit::Exit;

/// A command that could not run to its end.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The folder could not be read as a snapshot: reported as its diagnostic.
    Refused(host_native::Refused),
    /// `--rev` was given for a folder read as the files on disk.
    Unversioned {
        /// The site's folder.
        dir: PathBuf,
    },
    /// `--base` is no site address.
    Base {
        /// The address, as given.
        written: String,
        /// Why it is no address.
        source: engine::BaseUrlError,
    },
    /// The path given to `ink explain` is no path of a site.
    Path {
        /// The path, as given.
        written: String,
        /// Why it is no path.
        source: model::PathError,
    },
    /// A decision code given on the command line is no code.
    Code {
        /// Why.
        source: model::UnknownDecision,
    },
    /// A code given with a path is decided on the site, or one given without is decided per
    /// file.
    Scope {
        /// The code.
        code: model::AnyCode,
        /// What it was asked about.
        subject: model::Subject,
    },
    /// `--why` was given neither a code nor a path and a code.
    Words {
        /// How many words it was given.
        given: usize,
    },
    /// The decision asked about is never made, as for a file that is ignored.
    Undecided {
        /// The code.
        code: model::AnyCode,
        /// What it was asked about.
        subject: model::Subject,
    },
    /// The pin could not be written into ink.toml.
    Pin {
        /// Why.
        source: config::PinsError,
    },
    /// The path given to `ink explain` is no file of the folder.
    Unknown {
        /// The path.
        path: model::ContentPath,
    },
    /// A page's frontmatter cannot take or lose `publish`.
    Front {
        /// The page.
        path: model::ContentPath,
        /// Why.
        source: Box<format_markdown::Refusal>,
    },
    /// A file `ink select` was asked about is not a page whose frontmatter selects it.
    Unselectable {
        /// The file.
        path: model::ContentPath,
        /// Why not.
        why: crate::select::Unselectable,
    },
    /// `--folder` is no glob pattern.
    Folder {
        /// The pattern, as given.
        written: String,
        /// Why it is none.
        source: config::PatternError,
    },
    /// `include` could not be written into ink.toml.
    Include {
        /// Why.
        source: config::IncludeError,
    },
    /// `site.format` names a format plugin this ink does not hold.
    Format {
        /// The id.
        id: model::record::PluginId,
    },
    /// The output folder is one `ink build` will not clear or write into.
    Output(crate::out::Refusal),
    /// An entry of the deploy has no place in the output folder.
    Placed {
        /// Where it would go, as built from its route.
        file: String,
        /// Why that is no place.
        source: crate::out::RelativeError,
    },
    /// An entry's bytes are in no store.
    Blob {
        /// The entry's route.
        route: model::Route,
        /// Which blob is missing.
        source: engine::BlobMissing,
    },
    /// A file or folder in the output folder could not be read.
    Read {
        /// The file or folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The output folder or a file in it could not be written.
    Write {
        /// The folder or file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The previous build's manifest in the output folder does not parse.
    Manifest {
        /// The manifest's file.
        path: PathBuf,
        /// Why it does not parse.
        source: serde_json::Error,
    },
    /// The preview server could not listen or serve.
    Serve {
        /// What the system said.
        source: std::io::Error,
    },
    /// Stdout or stderr could not be written.
    Terminal {
        /// What the system said.
        source: std::io::Error,
    },
    /// A component `ink build --check` runs could not be read.
    Component {
        /// Its file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A component could not be loaded.
    Plugin {
        /// Why.
        source: host_wasmtime::Error,
    },
    /// A result could not be written as JSON.
    Json {
        /// Why it could not.
        source: serde_json::Error,
    },
}

impl Error {
    /// The exit code it ends the command with: a site's folder that is missing or cannot be
    /// listed is a usage error, like any path given wrong; what is wrong inside it is content.
    pub(crate) fn exit(&self) -> Exit {
        match self {
            Self::Refused(
                host_native::Refused::Root { .. }
                | host_native::Refused::Repository { .. }
                | host_native::Refused::Revision { .. },
            ) => Exit::Usage,
            Self::Refused(_) => Exit::Content,
            _ => Exit::Usage,
        }
    }

    /// What kind of error it is, as `--json` names it: a refused folder's diagnostic code, or
    /// `noun.reason` for the rest.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Refused(refused) => model::Diagnostic::from(refused).code(),
            Self::Unversioned { .. } => "source.rev.unversioned",
            Self::Base { .. } => "base.address.invalid",
            Self::Path { .. } => "explain.path.invalid",
            Self::Unknown { .. } => "explain.path.unknown",
            Self::Code { .. } => "decision.code.unknown",
            Self::Scope { .. } => "decision.code.scope",
            Self::Words { .. } => "decision.words.invalid",
            Self::Undecided { .. } => "decision.subject.undecided",
            Self::Pin { .. } => "pin.config.unwritable",
            Self::Front { .. } => "select.frontmatter.refused",
            Self::Unselectable { .. } => "select.file.unselectable",
            Self::Folder { .. } => "select.folder.invalid",
            Self::Include { .. } => "select.include.unwritable",
            Self::Format { .. } => "format.plugin.unknown",
            Self::Output(refusal) => refusal.kind(),
            Self::Placed { .. } => "output.entry.unplaced",
            Self::Blob { .. } => "output.blob.missing",
            Self::Read { .. } => "output.file.unreadable",
            Self::Write { .. } => "output.file.unwritable",
            Self::Manifest { .. } => "output.manifest.invalid",
            Self::Serve { .. } => "preview.serve.failed",
            Self::Terminal { .. } => "cli.terminal.unwritable",
            Self::Component { .. } => "component.file.unreadable",
            Self::Plugin { .. } => "component.load.failed",
            Self::Json { .. } => "cli.json.unwritable",
        }
    }
}

impl fmt::Display for Error {
    #[expect(clippy::too_many_lines, reason = "one arm for each variant of the error")]
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refused) => write!(out, "{refused}"),
            Self::Unversioned { dir } => {
                write!(out, "`--rev` names a commit, and {} is read as a folder, not a git repository", dir.display())
            }
            Self::Base { written, .. } => write!(out, "`{written}` is not an address a site is served at"),
            Self::Path { written, .. } => write!(out, "`{written}` is not a path in the folder"),
            Self::Unknown { path } => write!(out, "{path} is not a file of the folder"),
            Self::Code { .. } => write!(out, "no decision has that code"),
            Self::Scope { code, subject: model::Subject::File(path) } => {
                write!(out, "`{code}` is decided once for the site, not on {path}: give the code alone")
            }
            Self::Scope { code, .. } => write!(out, "`{code}` is decided per file: give the file's path before it"),
            Self::Words { given } => {
                write!(out, "a decision is named by a code, or a path and a code, not {given} words")
            }
            Self::Pin { .. } => write!(out, "the pin could not be written into ink.toml"),
            Self::Undecided { code, subject } => {
                write!(out, "`{code}` is not decided on {subject}; `ink explain` lists what is")
            }
            Self::Front { path, .. } => write!(out, "`publish` cannot be written into the frontmatter of {path}"),
            Self::Unselectable { path, why } => write!(out, "{path} cannot be selected: {why}"),
            Self::Folder { written, .. } => write!(out, "`{written}` is not a glob pattern"),
            Self::Include { .. } => write!(out, "the selection could not be written into ink.toml"),
            Self::Format { id } => {
                write!(
                    out,
                    "`site.format` names `{id}`, which this ink does not hold: it holds `ink:format-markdown` and `ink:format-obsidian`"
                )
            }
            Self::Output(refusal) => write!(out, "{refusal}"),
            Self::Placed { file, .. } => write!(out, "`{file}` is not a file `ink build` can write"),
            Self::Blob { route, .. } => write!(out, "the bytes of {route} are missing"),
            Self::Read { path, .. } => write!(out, "{} could not be read", path.display()),
            Self::Write { path, .. } => write!(out, "{} could not be written", path.display()),
            Self::Manifest { path, .. } => {
                write!(out, "the earlier build's manifest {} does not parse", path.display())
            }
            Self::Serve { .. } => write!(out, "the preview could not be served"),
            Self::Terminal { .. } => write!(out, "the terminal could not be written to"),
            Self::Component { path, .. } => write!(out, "the component {} could not be read", path.display()),
            Self::Plugin { .. } => write!(out, "a component could not be loaded"),
            Self::Json { .. } => write!(out, "the result could not be written as JSON"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(source) => Some(source),
            Self::Base { source, .. } => Some(source),
            Self::Path { source, .. } => Some(source),
            Self::Code { source } => Some(source),
            Self::Pin { source } => Some(source),
            Self::Front { source, .. } => Some(source.as_ref()),
            Self::Folder { source, .. } => Some(source),
            Self::Include { source } => Some(source),
            Self::Write { source, .. }
            | Self::Read { source, .. }
            | Self::Serve { source }
            | Self::Terminal { source }
            | Self::Component { source, .. } => Some(source),
            Self::Plugin { source } => Some(source),
            Self::Placed { source, .. } => Some(source),
            Self::Blob { source, .. } => Some(source),
            Self::Manifest { source, .. } | Self::Json { source } => Some(source),
            Self::Output(_)
            | Self::Format { .. }
            | Self::Unknown { .. }
            | Self::Unselectable { .. }
            | Self::Unversioned { .. }
            | Self::Scope { .. }
            | Self::Words { .. }
            | Self::Undecided { .. } => None,
        }
    }
}

impl From<std::io::Error> for Error {
    /// A failed write to stdout or stderr.
    fn from(source: std::io::Error) -> Self {
        Self::Terminal { source }
    }
}
