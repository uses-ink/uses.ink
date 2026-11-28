//! The error block: what stopped a command, its causes, a typed fix and the state it left behind
//! (decision 0061). As text it is `error[code]: what`, a `caused by` rail, `fix:` and a state
//! line; as JSON it is `uses.error/v1`, which gained `fix` and `state` beside `what` and `causes`.

use std::fmt;
use std::path::{Path, PathBuf};

use model::{Chain, Diagnostic};
use serde::Serialize;

use crate::command::select::Unselectable;
use crate::output::error::{AskError, ComponentError, Error, FolderError, ServeError, SourceError};
use crate::output::exit::Exit;
use crate::output::report::{Advice, Event, Label, Texts};
use crate::output::term::{Doc, Line, Render, Role, Tree};

/// What a command left behind when it stopped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum State {
    /// Nothing was written: the command stopped before it changed anything.
    Unchanged,
    /// A file or folder may be half written.
    Partial {
        /// Where.
        path: String,
    },
    /// An upload stopped after the site took this many files: they stay, and the next push
    /// sends only the rest.
    Uploaded {
        /// How many.
        sent: u64,
    },
    /// A build was queued on the site before the command stopped: the server keeps it going.
    Queued {
        /// The build.
        build: String,
    },
    /// The command could not say what it had done before it stopped.
    Unknown,
}

impl fmt::Display for State {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unchanged => write!(out, "nothing was changed"),
            Self::Partial { path } => write!(out, "`{path}` may be partly written"),
            Self::Uploaded { sent } => {
                write!(out, "the site holds {sent} files of this push: run it again to send the rest")
            }
            Self::Queued { build } => {
                write!(out, "build {build} is queued on the site and keeps going: `ink watch {build}` follows it")
            }
            Self::Unknown => write!(out, "what the command had written is not known: run it again"),
        }
    }
}

/// What to do about an error, as an instruction. A command inside backticks is the command to type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Remedy {
    ChooseOut,
    MoveAside,
    RemoveFile(PathBuf),
    DropRev,
    GiveAddress,
    GivePath,
    ListFiles,
    ListCodes,
    CodeAlone,
    PathFirst,
    NameDecision,
    FixCause,
    Unselect(Unselectable),
    GiveGlob,
    HoldFormat,
    Report,
    Access,
    Room,
    FreePort,
    FreeAddress,
    GiveDeploy,
    GiveData,
    Retry,
    Bundle,
    FixConfig,
    ListTopics,
    /// What a failure of the API says to do: a command in backticks, or a plain instruction.
    Said(String),
}

impl fmt::Display for Remedy {
    #[allow(clippy::too_many_lines, reason = "one arm for each remedy")]
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChooseOut => write!(out, "choose an empty folder outside the site with `-o`"),
            Self::MoveAside => write!(out, "move it out of the folder"),
            Self::RemoveFile(path) => {
                write!(out, "build into an empty folder with `-o`, or remove {} yourself", path.display())
            }
            Self::DropRev => write!(out, "drop `--rev`, or pass `--source git`"),
            Self::GiveAddress => {
                write!(
                    out,
                    "give the address the site is served at, with its scheme, as in `--base https://example.org`"
                )
            }
            Self::GivePath => {
                write!(out, "write the path as it is in the folder, relative to it, with `/` between its parts")
            }
            Self::ListFiles => write!(out, "`ink explain` lists every file of the folder and what is decided on it"),
            Self::ListCodes => write!(out, "`ink explain` lists every decision of the site with its code"),
            Self::CodeAlone => write!(out, "give the code alone, without a path"),
            Self::PathFirst => write!(out, "give the file's path before the code"),
            Self::NameDecision => write!(out, "name a decision as `CODE`, or as `PATH CODE`"),
            Self::FixCause => write!(out, "fix the cause above, then run the command again"),
            Self::Unselect(Unselectable::Ignored) => {
                write!(out, "remove what ignores it from `ink.toml`, or select another file")
            }
            Self::Unselect(Unselectable::Plain) => write!(out, "select the page that uses it"),
            Self::Unselect(_) => write!(out, "`--folder` selects any file by pattern"),
            Self::GiveGlob => write!(out, "write the pattern as a glob, such as `notes/**`"),
            Self::HoldFormat => {
                write!(out, "pin `site.format` to `ink:format-markdown` or `ink:format-obsidian`, or remove the pin")
            }
            Self::Report => write!(out, "this is a fault in ink: report it with the text above"),
            Self::Access => write!(out, "check that you may read it, then run the command again"),
            Self::Room => write!(out, "check that the folder is writable and has room, then run the command again"),
            Self::FreePort => write!(out, "free the port, or give another with `--port`"),
            Self::FreeAddress => write!(out, "give a free address with `--addr`"),
            Self::GiveDeploy => {
                write!(out, "give the folder `ink build` wrote into, or with `--host-mode folders` a folder of them")
            }
            Self::GiveData => write!(out, "give an empty folder, or the one `--data` used before"),
            Self::Retry => write!(out, "run the command again"),
            Self::FixConfig => write!(out, "fix the file, or remove the key it names"),
            Self::ListTopics => write!(out, "`ink help` lists the topics"),
            Self::Said(text) => write!(out, "{text}"),
            Self::Bundle => {
                write!(out, "run `cargo xtask component`, then build ink again (release builds require it)")
            }
        }
    }
}

impl Error {
    /// What to do about it, when there is something to say.
    #[allow(clippy::too_many_lines, reason = "one arm for each variant of the error")]
    pub(crate) fn remedy(&self) -> Option<Remedy> {
        Some(match self {
            Self::Source(SourceError::Refused(_)) => return None,
            Self::Source(source) => source.remedy(),
            Self::Ask(source) => source.remedy(),
            Self::Edit(_) => Remedy::FixCause,
            Self::Folder(source) => source.remedy(),
            Self::Serve(source) => source.remedy(),
            Self::Component(source) => source.remedy(),
            Self::Deploy { source } => Remedy::Said(source.remedy()),
            Self::Json { .. } | Self::Manual { .. } => Remedy::Report,
            Self::Terminal { .. } => Remedy::Retry,
            Self::Preferences { .. } => Remedy::FixConfig,
            Self::Help { .. } => Remedy::ListTopics,
        })
    }

    /// What it left behind. Only a write that failed or a report that could not be written leaves
    /// anything that is not known to be untouched.
    pub(crate) fn state(&self) -> State {
        match self {
            Self::Folder(FolderError::Write { path, .. }) => State::Partial { path: Self::shown(path) },
            Self::Terminal { .. } | Self::Json { .. } => State::Unknown,
            Self::Deploy { source } => match (source.stored(), source.queued()) {
                (Some(sent), _) => State::Uploaded { sent },
                (None, Some(build)) => State::Queued { build: build.to_owned() },
                (None, None) => State::Unchanged,
            },
            _ => State::Unchanged,
        }
    }

    fn shown(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }
}

impl SourceError {
    /// What to do about it. A refused folder says so as its own diagnostic, so it has no fix.
    fn remedy(&self) -> Remedy {
        match self {
            Self::Unversioned { .. } => Remedy::DropRev,
            Self::Base { .. } => Remedy::GiveAddress,
            Self::Format { .. } => Remedy::HoldFormat,
            Self::Refused(_) => Remedy::Report,
        }
    }
}

impl AskError {
    /// What to do about it.
    fn remedy(&self) -> Remedy {
        match self {
            Self::Path { .. } => Remedy::GivePath,
            Self::Unknown { .. } => Remedy::ListFiles,
            Self::Code { .. } | Self::Undecided { .. } => Remedy::ListCodes,
            Self::Scope { subject: model::Subject::File(_), .. } => Remedy::CodeAlone,
            Self::Scope { .. } => Remedy::PathFirst,
            Self::Words { .. } => Remedy::NameDecision,
            Self::Unselectable { why, .. } => Remedy::Unselect(*why),
            Self::Folder { .. } => Remedy::GiveGlob,
        }
    }
}

impl FolderError {
    /// What to do about it.
    fn remedy(&self) -> Remedy {
        match self {
            Self::Output(refusal) => refusal.remedy(),
            Self::Placed { .. } | Self::Blob { .. } => Remedy::Report,
            Self::Read { .. } => Remedy::Access,
            Self::Write { .. } => Remedy::Room,
            Self::Manifest { path, .. } => Remedy::RemoveFile(path.clone()),
        }
    }
}

impl ServeError {
    /// What to do about it.
    fn remedy(&self) -> Remedy {
        match self {
            Self::Edge { .. } | Self::Server { .. } => Remedy::Report,
            Self::Serve { .. } => Remedy::FreePort,
            Self::Listen { .. } => Remedy::FreeAddress,
            Self::Deploys { .. } => Remedy::GiveDeploy,
            Self::Product { .. } => Remedy::GiveData,
            Self::Run { .. } => Remedy::Retry,
        }
    }
}

impl ComponentError {
    /// What to do about it.
    fn remedy(&self) -> Remedy {
        match self {
            Self::Plugin { .. } => Remedy::Report,
            Self::Component { .. } => Remedy::Access,
            Self::Unbundled { .. } => Remedy::Bundle,
        }
    }
}

/// An error as a block: `uses.error/v1`.
#[derive(Debug, Serialize)]
pub(crate) struct Failure {
    schema: &'static str,
    exit: u8,
    /// The error's kind: a diagnostic code for a refused folder, `output.foreign` and the like
    /// for the rest.
    error: &'static str,
    what: String,
    /// Each cause, outermost first.
    causes: Vec<String>,
    /// What to do about it.
    fix: Option<String>,
    /// What it left behind.
    state: State,
    /// The refused folder's diagnostic, when it is one.
    diagnostic: Option<Event>,
    /// Lines that follow the block as they are, such as a usage line; text only.
    #[serde(skip)]
    trailer: Vec<String>,
}

impl Failure {
    const SCHEMA: &'static str = "uses.error/v1";

    /// The block for `error`, which ended the command with `exit`.
    pub(crate) fn of(error: &Error, exit: Exit) -> Self {
        let causes = Chain::new(error).causes().map(ToString::to_string).collect();
        let diagnostic = match error {
            Error::Source(SourceError::Refused(refused)) => {
                Some(Event::diagnostic(&Diagnostic::from(refused), Texts::NONE))
            }
            _ => None,
        };
        Self {
            schema: Self::SCHEMA,
            exit: exit.code(),
            error: error.kind(),
            what: error.to_string(),
            causes,
            fix: error.remedy().map(|remedy| remedy.to_string()),
            state: error.state(),
            diagnostic,
            trailer: Vec::new(),
        }
    }

    /// A command line clap refused: its first line is what, its `tip` the fix, and what follows
    /// the usage line is kept as it is.
    pub(crate) fn usage(error: &clap::Error) -> Self {
        let text = error.to_string();
        let mut lines = text.lines();
        let what = lines.next().unwrap_or_default();
        let what = what.strip_prefix("error: ").unwrap_or(what).replace('\'', "`");
        let rest: Vec<&str> = lines.collect();
        let tip =
            rest.iter().find_map(|line| line.trim_start().strip_prefix("tip: ")).map(|tip| tip.replace('\'', "`"));
        let fix = tip.or_else(|| Some("run the command with `--help` to see what it accepts".to_owned()));
        let trailer =
            rest.iter().skip_while(|line| !line.starts_with("Usage:")).map(|line| (*line).to_owned()).collect();
        Self {
            schema: Self::SCHEMA,
            exit: Exit::Usage.code(),
            error: "cli.usage",
            what,
            causes: Vec::new(),
            fix,
            state: State::Unchanged,
            diagnostic: None,
            trailer,
        }
    }
}

impl Render for Failure {
    fn render(&self, into: &mut Doc) {
        into.push(Label::error(self.error, &self.what).line());
        let mut causes = Tree::new(Line::plain("caused by")).indent(2);
        for cause in &self.causes {
            causes = causes.row(vec![Line::marked(Role::Plain, cause)]);
        }
        if !causes.is_empty() {
            causes.render(into);
        }
        if let Some(fix) = &self.fix {
            into.push(Advice(fix).line());
        }
        into.push(Line::new().pad(2).with(Role::Quiet, self.state.to_string()));
        if !self.trailer.is_empty() {
            into.blank();
            for line in &self.trailer {
                into.push(Line::plain(line));
            }
        }
    }
}
