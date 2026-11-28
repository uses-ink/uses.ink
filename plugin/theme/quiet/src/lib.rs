//! `ink:theme-quiet`, the default theme. Deliberately plain: semantic HTML5 written through
//! `unspliced`, one small stylesheet (system fonts, a readable measure, light and dark from the
//! reader's setting), a stylesheet with a class for each highlighting capture, and no JavaScript
//! ever. Its real design is the owner's (open.md T1).
//!
//! Code is highlighted by the host: [`Quiet`] is generic over a
//! [`Highlight`](model::highlight::Highlight), the capability of decision 0039, and links no
//! highlighter of its own.
//!
//! It implements [`engine::Theme`] directly: this crate is the theme's native core, and `guest/`
//! exports it as the `ink:theme` world of a wasm component (decision 0041).
//!
//! `document` writes a page's document as one [`model::Visit`]; `page` writes the page around
//! it.

mod document;
mod page;
mod style;

use engine::{BaseUrl, File, Html, Reads, Sink, Theme};
use model::highlight::Highlight;
use model::{Diagnostic, Hash, MediaType, Page, PageKind, Problem, Route, Site, Subject};

use document::Source;
use page::Layout;

/// The theme, highlighting code through `H`, the host's capability.
#[derive(Debug)]
pub struct Quiet<H> {
    base: BaseUrl,
    highlight: H,
}

impl<H: Highlight> Quiet<H> {
    /// The theme for a site served at `base`, for canonical links and `og:url`, its code
    /// highlighted by `highlight`.
    pub fn new(base: BaseUrl, highlight: H) -> Self {
        Self { base, highlight }
    }
}

impl<H> Quiet<H> {
    /// The theme's own files: its stylesheet and its code stylesheet, the same for every site.
    pub fn files() -> Vec<File> {
        let mut files = Vec::new();
        let style = style::quiet::SHEET.text().as_bytes().to_vec();
        if let Ok(route) = page::STYLE.parse::<Route>() {
            files.push(File { route, media: MediaType::Css, bytes: style });
        }
        let code = style::code::SHEET.text().as_bytes().to_vec();
        if let Ok(route) = page::CODE.parse::<Route>() {
            files.push(File { route, media: MediaType::Css, bytes: code });
        }
        files
    }
}

/// The theme's own code, as compiled in, for [`Theme::identity`].
#[derive(Clone, Copy, Debug)]
struct Code;

impl Code {
    /// Every source file of this crate. A test holds the list to the folder.
    const SOURCES: &'static [&'static [u8]] = &[
        include_bytes!("lib.rs"),
        include_bytes!("page.rs"),
        include_bytes!("style.rs"),
        include_bytes!("document.rs"),
        include_bytes!("document/code.rs"),
        include_bytes!("document/math.rs"),
        include_bytes!("document/math/allow.rs"),
        include_bytes!("document/note.rs"),
        include_bytes!("document/picture.rs"),
    ];

    /// The versions of everything the workspace builds with.
    const LOCK: &'static [u8] = include_bytes!("../../../../Cargo.lock");
}

impl<H: Highlight> Theme for Quiet<H> {
    fn name(&self) -> &'static str {
        "ink:theme-quiet"
    }

    fn assets(&self) -> Vec<File> {
        Self::files()
    }

    /// What its pages are made from besides the page and the site: its name and release, its
    /// stylesheets, its own code as compiled in (a change to how a page is written changes the
    /// key, with no version bump), and the workspace's `Cargo.lock`, which holds the versions of
    /// `unspliced`, the highlighter's grammars and every other crate that reaches a page's
    /// bytes. It has no tokens yet; when it has, they join this hash.
    fn identity(&self) -> Option<Hash> {
        let mut parts = vec![self.name().as_bytes(), env!("CARGO_PKG_VERSION").as_bytes(), Code::LOCK];
        parts.extend(Code::SOURCES);
        let files = Self::files();
        parts.extend(files.iter().map(|file| file.bytes.as_slice()));
        // each part is length-prefixed, so two lists of parts never join into the same bytes
        let mut joined = Vec::new();
        for part in parts {
            joined.extend((part.len() as u64).to_le_bytes());
            joined.extend(part);
        }
        Some(Hash::of(joined))
    }

    /// The home lists the posts and, on a generated home, every page; every other page shows
    /// the site's navigation, its links and its images.
    fn reads(&self, page: &Page) -> Reads {
        if page.kind == PageKind::Home { Reads::Listed } else { Reads::Linked }
    }

    /// A page the theme could not write whole is `render.page.failed`: a bug in the theme.
    fn render<R: Sink + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        let mut html = String::new();
        let source = Source { page, site, highlight: &self.highlight };
        let layout = Layout { source, base: &self.base };
        match layout.write(&mut html, record) {
            Ok(()) => Ok(Html::from(html)),
            Err(unwritten) => {
                let problem = Problem::RenderPageFailed { reason: unwritten.to_string() };
                Err(Diagnostic::new(problem).on(Subject::File(page.file.clone())))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::Code;

    fn sources(dir: &Path, found: &mut Vec<Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(std::fs::read(&path).unwrap());
            }
        }
    }

    #[test]
    fn the_identity_holds_every_source_file_of_the_theme() {
        let mut found = Vec::new();
        sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
        let compiled: Vec<&[u8]> = Code::SOURCES.to_vec();
        assert_eq!(found.len(), compiled.len(), "a source file is missing from `Code::SOURCES`, or listed twice");
        for file in &found {
            assert!(compiled.contains(&file.as_slice()));
        }
    }
}
