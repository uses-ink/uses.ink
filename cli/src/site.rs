//! A folder read, as the files on disk or as a git repository at one commit, its `ink.toml`
//! parsed, and the site interpreted from both.

use std::collections::BTreeMap;
use std::path::Path;

use config::{Config, Parsed};
use engine::{BaseUrl, Blobs, Engine, Formats, NoPins, RenderSettings, Theme, Understood};
use host_image::Pixels;
use host_native::{Folder, Grammars, Memory, Read, Repository, Revision};
use model::{Announcement, ContentPath, Diagnostic, Hash, Manifest, Notice, Problem, Snapshot, Subject, Version};
use theme_quiet::Quiet;

use crate::args::{Kind, Origin};
use crate::error::Error;
use crate::format::Markdown;
use crate::pins::Configured;

/// The output folder when `ink build` is given no `-o`, inside the site's folder.
pub(crate) const DEFAULT_OUT: &str = ".ink/out";

/// The config file's path in the snapshot.
const CONFIG: &str = "ink.toml";

/// A folder to interpret.
#[derive(Debug)]
pub(crate) struct Source<'a> {
    pub(crate) dir: &'a Path,
    /// Folders in it holding this build's output, not read.
    pub(crate) output: Vec<ContentPath>,
    /// Whether it is read as a folder or a repository, and at which commit.
    origin: &'a Origin,
}

/// How a folder is read, once [`Origin`] and the folder itself have said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reading {
    Folder,
    /// A repository; `chosen` when nothing asked for it but the folder's `.git`, so it is said.
    Git {
        chosen: bool,
    },
}

/// Everything known after interpreting a folder.
#[derive(Debug)]
pub(crate) struct Loaded {
    pub(crate) snapshot: Snapshot,
    pub(crate) store: Memory,
    /// The engine's decisions and site, `ink.toml`'s problems and the folder's notices among its
    /// diagnostics and notices.
    pub(crate) understood: Understood,
}

impl<'a> Source<'a> {
    /// The folder at `dir`, read as `origin` says; its default output folder, `.ink/out`, is not
    /// read when it exists.
    pub(crate) fn new(dir: &'a Path, origin: &'a Origin) -> Self {
        let output = DEFAULT_OUT.parse().ok().filter(|_| dir.join(DEFAULT_OUT).is_dir()).into_iter().collect();
        Self { dir, output, origin }
    }

    /// How the folder is read: as `--source` says, else as a repository when it holds `.git`.
    /// `--rev` names a commit, so it needs a repository.
    fn reading(&self) -> Result<Reading, Error> {
        let versioned = std::fs::symlink_metadata(self.dir.join(".git")).is_ok();
        match (self.origin.source, self.origin.rev.is_some(), versioned) {
            (Some(Kind::Folder), true, _) | (None, true, false) => {
                Err(Error::Unversioned { dir: self.dir.to_path_buf() })
            }
            (Some(Kind::Folder), false, _) | (None, false, false) => Ok(Reading::Folder),
            (Some(Kind::Git), _, _) | (None, true, true) => Ok(Reading::Git { chosen: false }),
            (None, false, true) => Ok(Reading::Git { chosen: true }),
        }
    }

    /// The folder read into `store`, with what the read has to say.
    fn read(&self, store: &mut Memory) -> Result<(Read, Vec<Notice>), Error> {
        let output = self.output.clone();
        let reading = self.reading()?;
        let read = match reading {
            Reading::Folder => Folder { output, ..Folder::new(self.dir) }.read(store),
            Reading::Git { .. } => {
                let revision = self.origin.rev.clone().map_or(Revision::Head, Revision::Spec);
                Repository { revision, output, ..Repository::new(self.dir) }.read(store)
            }
        }
        .map_err(Error::Refused)?;
        let mut notices = Vec::new();
        if let (Reading::Git { chosen: true }, Version::GitCommit(commit)) = (reading, read.snapshot.version()) {
            notices.push(Notice::new(Announcement::SourceGitChosen { commit: commit.to_string() }));
        }
        notices.extend(read.notices());
        notices.extend(read.snapshot.observed().lacks.iter().filter_map(model::Lack::notice));
        Ok((read, notices))
    }

    pub(crate) fn load(self) -> Result<Loaded, Error> {
        self.interpret((Markdown,))
    }

    /// The source interpreted through `formats`.
    pub(crate) fn interpret<F: Formats>(self, formats: F) -> Result<Loaded, Error> {
        let mut store = Memory::default();
        let (read, said) = self.read(&mut store)?;
        let snapshot = read.snapshot;
        let mut parsed = Parsed::default();
        let config_path: Option<ContentPath> = CONFIG.parse().ok();
        let entry = config_path.as_ref().and_then(|path| snapshot.entry(path).map(|entry| (path, entry.blob)));
        if let Some((path, blob)) = entry {
            let subject = Subject::File(path.clone());
            match store.read(&blob).map(|bytes| String::from_utf8(bytes.into_owned())) {
                Ok(Ok(text)) => {
                    parsed = Config::parse(&text);
                    for diagnostic in &mut parsed.diagnostics {
                        diagnostic.subject = Some(subject.clone());
                    }
                }
                Ok(Err(error)) => {
                    let problem = Problem::SourceFileEncoding { at: error.utf8_error().valid_up_to() as u64 };
                    parsed.diagnostics.push(Diagnostic::new(problem).on(subject));
                }
                Err(_) => {
                    let problem = Problem::SourceBlobMissing { path: path.clone() };
                    parsed.diagnostics.push(Diagnostic::new(problem).on(subject));
                }
            }
        }
        let Parsed { config, diagnostics } = parsed;
        let mut understood = Engine::new(&store, Configured(&config), formats, ()).interpret(&snapshot);
        let stops = diagnostics.iter().any(|d| d.severity() == model::Severity::Error);
        understood.diagnostics.splice(0..0, diagnostics);
        if stops {
            understood.site = None;
        }
        understood.notices.splice(0..0, said);
        Ok(Loaded { snapshot, store, understood })
    }
}

/// The address a site is served at, and whether it was given or chosen.
#[derive(Clone, Debug)]
pub(crate) struct Address {
    pub(crate) base: BaseUrl,
    /// `build.address.default` when no address was given.
    pub(crate) chosen: Option<Notice>,
}

impl Address {
    /// The address when nothing says where the site is served: the preview's own.
    const DEFAULT: &'static str = "http://127.0.0.1:4000";

    /// `--base`, or the default with a notice saying so.
    pub(crate) fn from_flag(flag: Option<&str>) -> Result<Self, Error> {
        let written = flag.unwrap_or(Self::DEFAULT);
        let base: BaseUrl = written.parse().map_err(|source| Error::Base { written: written.to_owned(), source })?;
        let chosen =
            flag.is_none().then(|| Notice::new(Announcement::BuildAddressDefault { address: written.to_owned() }));
        Ok(Self { base, chosen })
    }
}

/// A site rendered, with everything said on the way from the folder to the deploy.
#[derive(Debug)]
pub(crate) struct Rendering {
    /// `None` when interpreting stopped on an error.
    pub(crate) deploy: Option<Manifest>,
    pub(crate) blobs: BTreeMap<Hash, Vec<u8>>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) notices: Vec<Notice>,
}

impl Loaded {
    /// Renders the site through `ink:theme-quiet`, its images through the host's image work, if
    /// interpreting gave one.
    pub(crate) fn render(&self, address: &Address, images: &Pixels) -> Rendering {
        self.through(address, &Quiet::new(address.base.clone(), Grammars::default()), images)
    }

    /// Renders the site through `theme`, its images through `images`, if interpreting gave one.
    pub(crate) fn through<T: Theme>(&self, address: &Address, theme: &T, images: &Pixels) -> Rendering {
        let understood = &self.understood;
        let mut diagnostics = understood.diagnostics.clone();
        let mut notices = understood.notices.clone();
        notices.extend(address.chosen.clone());
        let Some(site) = &understood.site else {
            return Rendering { deploy: None, blobs: BTreeMap::new(), diagnostics, notices };
        };
        let engine = Engine::new(&self.store, NoPins, (), theme);
        let rendered = engine.render(site, &RenderSettings { base: address.base.clone() }, images);
        diagnostics.extend(rendered.diagnostics);
        notices.extend(rendered.notices);
        Rendering { deploy: Some(rendered.manifest), blobs: rendered.blobs, diagnostics, notices }
    }
}
