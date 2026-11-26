//! What the tests build sites from: an in-memory snapshot, a stand-in format that reads a few
//! lines, a stand-in theme that writes a few tags, and pins held in a list.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use engine::{
    BlobMissing, Blobs, Engine, File, Format, Html, Patterns, Pin, Pins, Record, RenderSettings, Theme, Understood,
};
use model::code::{AnyDecision, Code};
use model::decision::{self, Decision};
use model::{
    Ability, Announcement, Block, Bytes, ContentPath, Diagnostic, Document, Entry, Evidence, FileFacts, Front, Hash,
    ImageSource, Inline, Known, Level, LinkTarget, Notice, Observed, Page, PageKind, Problem, Read, Site, Snapshot,
    Span, Spanned, Subject, Tag, UnknownReason, Version, When,
};

/// A snapshot written file by file.
#[derive(Debug, Default)]
pub struct Tree {
    files: BTreeMap<String, Vec<u8>>,
    facts: Vec<FileFacts>,
    observed: Observed,
    missing: Vec<String>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_owned(), text.as_bytes().to_vec());
        self
    }

    /// A file whose bytes the store does not hold.
    pub fn lost(mut self, path: &str) -> Self {
        self.missing.push(path.to_owned());
        self.file(path, "lost")
    }

    pub fn created(mut self, path: &str, created: Known<model::Timestamp>) -> Self {
        let unknown = Known::Unknown { reason: UnknownReason::NoHistory };
        self.facts.push(FileFacts::new(path.parse().unwrap(), created, unknown));
        self
    }

    pub fn private(mut self) -> Self {
        self.observed = Observed::new(vec![Ability::Snapshot, Ability::Private], vec![]);
        self
    }

    pub fn snapshot(&self) -> (Snapshot, Store) {
        let mut store = Store::default();
        let tree = self
            .files
            .iter()
            .map(|(path, bytes)| {
                let hash = Hash::of(bytes);
                if !self.missing.contains(path) {
                    store.0.insert(hash, bytes.clone());
                }
                Entry::new(path.parse().unwrap(), hash, Bytes(bytes.len() as u64))
            })
            .collect();
        let version = Version::Folder { hash: Hash::of(b"tree") };
        (Snapshot::new(version, tree, self.facts.clone(), self.observed.clone()).unwrap(), store)
    }

    pub fn understand(&self, pins: &Pinned) -> Understood {
        self.read_with(pins, Lines)
    }

    /// Interprets the tree through `format`.
    pub fn read_with(&self, pins: &impl Pins, format: impl Format) -> Understood {
        let (snapshot, store) = self.snapshot();
        Engine::new(&store, pins, (format,), ()).interpret(&snapshot)
    }
}

/// Blobs by hash.
#[derive(Debug, Default)]
pub struct Store(BTreeMap<Hash, Vec<u8>>);

impl Blobs for Store {
    fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing> {
        self.0.get(hash).map(|bytes| Cow::Borrowed(bytes.as_slice())).ok_or(BlobMissing { hash: *hash })
    }
}

/// Pins, includes and excludes, held in lists, the pins erased as storage holds them. A pattern
/// matches a path it prefixes.
#[derive(Debug, Default)]
pub struct Pinned {
    pins: Vec<AnyDecision>,
    include: Vec<String>,
    exclude: Vec<String>,
}

pub fn span() -> Span {
    Span::new(0, 1).unwrap()
}

impl Pinned {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn pin<C: Code>(mut self, subject: C::Subject, value: C::Value) -> Self {
        let decision = Decision::<C>::new(subject, value, Evidence::Config { span: span() });
        self.pins.push(decision.into());
        self
    }

    pub fn site<C: Code<Subject = decision::Site>>(self, value: C::Value) -> Self {
        self.pin::<C>(decision::Site, value)
    }

    pub fn file<C: Code<Subject = decision::File>>(self, path: &str, value: C::Value) -> Self {
        self.pin::<C>(decision::File(path.parse().unwrap()), value)
    }

    pub fn include(mut self, prefix: &str) -> Self {
        self.include.push(prefix.to_owned());
        self
    }

    pub fn exclude(mut self, prefix: &str) -> Self {
        self.exclude.push(prefix.to_owned());
        self
    }
}

impl Pins for Pinned {
    fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>> {
        let found = self.pins.iter().find_map(|d| d.get::<C>().filter(|d| &d.subject == subject))?;
        Some(Pin { value: found.value.clone(), span: span() })
    }
}

impl Patterns for Pinned {
    fn included(&self, path: &ContentPath) -> Option<Span> {
        self.include.iter().any(|p| path.as_str().starts_with(p.as_str())).then(span)
    }

    fn excluded(&self, path: &ContentPath) -> Option<Span> {
        self.exclude.iter().any(|p| path.as_str().starts_with(p.as_str())).then(span)
    }
}

/// A stand-in format for `.md` files: `key: value` lines between `---` lines, `# heading`,
/// `[text](target)` and `![alt](target)` lines, and paragraphs of anything else.
#[derive(Debug)]
pub struct Lines;

impl Format for Lines {
    fn handles(&self, path: &ContentPath) -> bool {
        path.extension() == Some("md")
    }

    fn read(&self, _path: &ContentPath, bytes: &[u8]) -> Read {
        let text = std::str::from_utf8(bytes).unwrap();
        let mut front = Front::default();
        let mut heading = None;
        let mut blocks = Vec::new();
        let mut problems = Vec::new();
        let mut in_front = false;
        let mut at = 0;
        for line in text.split_inclusive('\n') {
            let span = Span::new(at, at + line.len()).unwrap();
            at += line.len();
            let line = line.trim_end();
            if line == "---" {
                in_front = !in_front;
            } else if in_front {
                if let Err(key) = key(&mut front, line, span) {
                    problems.push(Problem::FrontmatterSyntaxInvalid { reason: key });
                }
            } else if let Some(text) = line.strip_prefix("# ") {
                heading.get_or_insert(Spanned::new(text.to_owned(), span));
                let anchor = model::Anchor::of_heading(text).unwrap();
                let inlines = vec![Inline::Text(text.to_owned())];
                blocks.push(Block::Heading { level: Level::new(1).unwrap(), anchor, inlines });
            } else if !line.is_empty() {
                blocks.push(Block::Paragraph(vec![inline(line)]));
            }
        }
        let mut read = Read::new(Document::new(blocks), front, heading);
        read.diagnostics = problems.into_iter().map(Diagnostic::new).collect();
        read
    }
}

fn at<T>(value: T, span: Span) -> Option<Spanned<T>> {
    Spanned::new(value, span).into()
}

fn key(front: &mut Front, line: &str, span: Span) -> Result<(), String> {
    let (key, value) = line.split_once(": ").ok_or_else(|| line.to_owned())?;
    match key {
        "title" => front.title = at(value.to_owned(), span),
        "date" => front.date = at(when(value), span),
        "updated" => front.updated = at(when(value), span),
        "slug" => front.slug = at(value.parse().unwrap(), span),
        "tags" => front.tags = at(value.split(", ").map(|t| Tag::new(t).unwrap()).collect(), span),
        "publish" => front.publish = at(value == "true", span),
        "draft" => front.draft = at(value == "true", span),
        "description" => front.description = at(value.to_owned(), span),
        "lang" => front.language = at(value.parse().unwrap(), span),
        "type" => front.kind = at(if value == "post" { PageKind::Post } else { PageKind::Page }, span),
        other => return Err(other.to_owned()),
    }
    Ok(())
}

fn when(text: &str) -> When {
    text.parse().map_or_else(|_| When::Instant(text.parse().unwrap()), When::Day)
}

fn inline(line: &str) -> Inline {
    let target = |rest: &str| {
        rest.split_once("](").map(|(text, target)| (text.to_owned(), target.trim_end_matches(')').to_owned()))
    };
    if let Some((alt, written)) = line.strip_prefix("![").and_then(target) {
        return Inline::Image { source: ImageSource::Unresolved { written }, alt };
    }
    if let Some((text, written)) = line.strip_prefix('[').and_then(target) {
        return Inline::Link { target: LinkTarget::Unresolved { written }, children: vec![Inline::Text(text)] };
    }
    Inline::Text(line.to_owned())
}

/// A stand-in theme: the title, then each paragraph's link or image as where it points.
#[derive(Debug, Default)]
pub struct Plain {
    pub files: Vec<File>,
    /// Pages it refuses, by title, each after a notice that it was asked to.
    pub refuses: Vec<String>,
}

impl Theme for Plain {
    fn name(&self) -> &'static str {
        "plain"
    }

    fn assets(&self) -> Vec<File> {
        self.files.clone()
    }

    fn render<R: Record + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        if self.refuses.contains(&page.title) {
            record.notice(Notice::new(Announcement::CodeLanguageUnknown { language: "refused".to_owned() }));
            return Err(Diagnostic::new(Problem::HtmlElementStripped { element: "refused".to_owned() }));
        }
        let mut html = format!("<!doctype html><title>{}</title>", page.title);
        for block in &page.body.blocks {
            if let Block::Paragraph(inlines) = block {
                for inline in inlines {
                    let _infallible = write!(html, "<p>{}</p>", shown(inline, site));
                }
            }
        }
        Ok(Html::from(html))
    }
}

fn shown(inline: &Inline, site: &Site) -> String {
    match inline {
        Inline::Link { target: LinkTarget::Page { id, anchor }, .. } => {
            let route = site.page(*id).map(|p| p.route.to_string()).unwrap();
            anchor.as_ref().map_or(route.clone(), |a| format!("{route}#{a}"))
        }
        Inline::Link { target: LinkTarget::Anchor(a), .. } => format!("#{a}"),
        Inline::Link { target: LinkTarget::External(url), .. } => url.to_string(),
        Inline::Link { target: LinkTarget::Unresolved { written }, .. } => format!("text:{written}"),
        Inline::Image { source: ImageSource::Asset(path), .. } => {
            site.assets().iter().find(|a| &a.file == path).map(|a| a.route.to_string()).unwrap()
        }
        Inline::Image { source, .. } => format!("{source:?}"),
        Inline::Text(text) => text.clone(),
        other => format!("{other:?}"),
    }
}

pub fn settings() -> RenderSettings {
    RenderSettings { base: "https://ink.uses.ink".parse().unwrap() }
}

pub fn path(text: &str) -> ContentPath {
    text.parse().unwrap()
}

pub fn subject(text: &str) -> Subject {
    Subject::File(path(text))
}

/// The codes of every diagnostic.
pub fn codes(understood: &Understood) -> Vec<&'static str> {
    understood.diagnostics.iter().map(Diagnostic::code).collect()
}

/// The decision of `C` on `subject`.
pub fn decided<C: Code>(understood: &Understood, subject: &C::Subject) -> Decision<C> {
    understood.decisions.iter().find_map(|d| d.get::<C>().filter(|d| &d.subject == subject)).cloned().unwrap()
}

/// The decision of `C` on the file at `path`.
pub fn on<C: Code<Subject = decision::File>>(understood: &Understood, path: &str) -> Decision<C> {
    decided::<C>(understood, &decision::File(self::path(path)))
}

/// The decision of `C` on the site.
pub fn site<C: Code<Subject = decision::Site>>(understood: &Understood) -> Decision<C> {
    decided::<C>(understood, &decision::Site)
}
