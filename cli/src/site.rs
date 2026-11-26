//! A folder read, its `ink.toml` parsed, and the site interpreted from both.

use std::collections::BTreeMap;
use std::path::Path;

use config::{Config, Parsed};
use engine::{BaseUrl, Blobs, RenderSettings, Understood};
use host_native::{Folder, Memory};
use model::{Announcement, ContentPath, Diagnostic, Hash, Manifest, Notice, Problem, Snapshot, Subject};
use theme_quiet::Quiet;

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
    /// The folder at `dir`; its default output folder, `.ink/out`, is not read when it exists.
    pub(crate) fn new(dir: &'a Path) -> Self {
        let output = DEFAULT_OUT.parse().ok().filter(|_| dir.join(DEFAULT_OUT).is_dir()).into_iter().collect();
        Self { dir, output }
    }

    pub(crate) fn load(self) -> Result<Loaded, Error> {
        let mut store = Memory::default();
        let folder = Folder { output: self.output, ..Folder::new(self.dir) };
        let read = folder.read(&mut store).map_err(Error::Refused)?;
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
        let mut understood = engine::interpret(&snapshot, &store, &Configured(&config), &[&Markdown]);
        let stops = diagnostics.iter().any(|d| d.severity() == model::Severity::Error);
        understood.diagnostics.splice(0..0, diagnostics);
        if stops {
            understood.site = None;
        }
        let skipped = read.skipped.into_iter().map(|skipped| {
            Notice::new(Announcement::SourceOutputSkipped { path: skipped.path.clone() })
                .on(Subject::File(skipped.path))
        });
        understood.notices.splice(0..0, skipped);
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
    /// Renders the site through `ink:theme-quiet`, if interpreting gave one.
    pub(crate) fn render(&self, address: &Address) -> Rendering {
        let understood = &self.understood;
        let mut diagnostics = understood.diagnostics.clone();
        let mut notices = understood.notices.clone();
        notices.extend(address.chosen.clone());
        let Some(site) = &understood.site else {
            return Rendering { deploy: None, blobs: BTreeMap::new(), diagnostics, notices };
        };
        let theme = Quiet::new(address.base.clone());
        let rendered = engine::render(site, &theme, &RenderSettings { base: address.base.clone() });
        let said = theme.said();
        diagnostics.extend(rendered.diagnostics);
        diagnostics.extend(said.diagnostics);
        notices.extend(rendered.notices);
        notices.extend(said.notices);
        Rendering { deploy: Some(rendered.manifest), blobs: rendered.blobs, diagnostics, notices }
    }
}
