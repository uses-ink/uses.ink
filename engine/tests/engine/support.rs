//! What the tests build sites from: an in-memory snapshot, a stand-in format that reads a few
//! lines, a stand-in theme that writes a few tags, and pins held in a list.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use engine::image::{Encoded, Kept, Prepared, Source, Variants};
use engine::{
    BlobMissing, Blobs, Engine, File, Format, Html, Images, NoPins, Patterns, Pin, Pins, Record, RenderSettings,
    Rendered, Theme, Understood,
};
use model::code::{AnyDecision, Code};
use model::decision::{self, Decision};
use model::{
    Ability, Announcement, Block, Bytes, ContentPath, Diagnostic, Dimensions, Document, Entry, Evidence, FileFacts,
    Front, Hash, ImageSource, Inline, Known, Level, LinkTarget, MediaType, Notice, Observed, Page, PageKind, Problem,
    Read, Site, Snapshot, Span, Spanned, Subject, Tag, UnknownReason, Version, When,
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

    /// A source with history, lacking it before a shallow clone's boundary.
    pub fn shallow(mut self) -> Self {
        let bound = "2023-01-01T00:00:00Z".parse().unwrap();
        let gap = model::Gap::Shallow { commit: model::Oid::try_from("a".repeat(40)).unwrap(), bound };
        let lack = model::Lack::new(Ability::History, model::AnyCode::Date, gap);
        self.observed = Observed::new(vec![Ability::Snapshot, Ability::History], vec![lack]);
        self
    }

    /// A source with its whole history.
    pub fn deep(mut self) -> Self {
        self.observed = Observed::new(vec![Ability::Snapshot, Ability::History], vec![]);
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

    /// The record of a build of the tree under `pins`, by this engine with no plugin.
    pub fn record(&self, pins: &Pinned) -> model::record::Build {
        let (snapshot, _) = self.snapshot();
        let inputs = model::record::Inputs::of(&snapshot, engine::RELEASE.into(), Vec::new(), None);
        self.understand(pins).record(&snapshot, inputs)
    }

    /// Interprets the tree, tracing the ladder of `code` on `subject`.
    pub fn trace(&self, pins: &Pinned, code: model::AnyCode, subject: model::Subject) -> Understood {
        let (snapshot, store) = self.snapshot();
        Engine::new(&store, pins, (Lines,), ()).trace(&snapshot, code, subject)
    }

    /// Interprets the tree through `format`.
    pub fn read_with(&self, pins: &impl Pins, format: impl Format) -> Understood {
        let (snapshot, store) = self.snapshot();
        Engine::new(&store, pins, (format,), ()).interpret(&snapshot)
    }
}

impl Tree {
    /// Interprets the tree and renders it through `theme`, its images through [`Stand`].
    pub fn render(&self, theme: &impl Theme) -> Rendered {
        let (_, store) = self.snapshot();
        let site = self.understand(&Pinned::none()).site.unwrap();
        Engine::new(&store, NoPins, (), theme).render(&site, &settings(), &Stand)
    }
}

/// A stand-in for the host's image work, reading an image's bytes as what to do: `animated`
/// keeps it, `broken` refuses it, `clear` has transparency (no placeholder); anything else is
/// 40 by 30 pixels, copied as AVIF and JPEG at widths 20 and 40, its bytes the source's with
/// the format and width after them.
#[derive(Debug, Default)]
pub struct Stand;

impl Stand {
    fn copy(source: &Source<'_>, media: MediaType, width: u32) -> Encoded {
        let mut bytes = source.bytes.to_vec();
        bytes.extend(format!(" {} {width}", media.essence()).into_bytes());
        Encoded::new(media, Dimensions::new(width, width * 3 / 4), bytes)
    }

    #[expect(clippy::result_large_err, reason = "a refusal is the diagnostic the engine records")]
    fn prepare(source: &Source<'_>) -> Result<Prepared, Diagnostic> {
        let dimensions = Dimensions::new(40, 30);
        match source.bytes {
            b"animated" => Ok(Prepared::Kept { dimensions, reason: Kept::Animated }),
            b"broken" => Err(Diagnostic::new(Problem::ImageDecodeFailed {
                path: source.file.clone(),
                reason: "broken".to_owned(),
            })),
            bytes => {
                let copies = [MediaType::Avif, MediaType::Jpeg]
                    .into_iter()
                    .flat_map(|media| [20, 40].map(|width| Self::copy(source, media, width)))
                    .collect();
                let placeholder = (bytes != b"clear").then(|| Self::copy(source, MediaType::Png, 4));
                Ok(Prepared::Varied(Variants::new(dimensions, copies, placeholder)))
            }
        }
    }
}

impl Images for Stand {
    type Refusal = Diagnostic;

    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Diagnostic>> {
        sources.iter().map(Self::prepare).collect()
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
/// `[text](target)` and `![alt](target)` lines, `![[target]]` lines as embeds, `^id` lines as
/// block ids, `~ text ![[target]] text` as a paragraph with an embed among its text, `#~ text
/// ![[target]]` as a heading with one, `#t a, b` as tags written in the text, and paragraphs of anything else.
#[derive(Debug)]
pub struct Lines;

impl Lines {
    /// The tags of the `#t a, b` lines, each at its line.
    fn inline_tags(text: &str) -> Vec<Spanned<Tag>> {
        let mut at = 0;
        let mut found = Vec::new();
        for line in text.split_inclusive('\n') {
            let span = Span::new(at, at + line.len()).unwrap();
            at += line.len();
            let tags = line.trim_end().strip_prefix("#t ").into_iter().flat_map(|tags| tags.split(", "));
            found.extend(tags.map(|tag| Spanned::new(Tag::new(tag).unwrap(), span)));
        }
        found
    }
}

impl Format for Lines {
    fn handles(&self, path: &ContentPath) -> bool {
        path.extension() == Some("md")
    }

    fn read(&self, _path: &ContentPath, bytes: &[u8]) -> Read {
        let text = std::str::from_utf8(bytes).unwrap();
        let mut front = Front::default();
        let mut heading = None;
        let (mut blocks, mut problems) = (Vec::new(), Vec::new());
        let (mut in_front, mut at) = (false, 0);
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
            } else if let Some(text) = line.strip_prefix("~ ") {
                blocks.push(Block::Paragraph(among(text)));
            } else if let Some(text) = line.strip_prefix("#~ ") {
                blocks.push(headed(text));
            } else if let Some(text) = line.strip_prefix("# ") {
                heading.get_or_insert(Spanned::new(text.to_owned(), span));
                let anchor = model::Anchor::of_heading(text).unwrap();
                let inlines = vec![Inline::Text(text.to_owned())];
                blocks.push(Block::Heading { level: Level::new(1).unwrap(), anchor, inlines });
            } else if let Some(written) = line.strip_prefix("![[").and_then(|rest| rest.strip_suffix("]]")) {
                blocks.push(Block::Embed {
                    target: LinkTarget::Unresolved { written: written.to_owned() },
                    blocks: Vec::new(),
                });
            } else if let Some(id) = line.strip_prefix('^') {
                blocks.push(Block::Paragraph(vec![Inline::Anchor(model::Anchor::of_heading(id).unwrap())]));
            } else if !line.is_empty() {
                blocks.push(Block::Paragraph(vec![inline(line)]));
            }
        }
        let mut read = Read::new(Document::new(blocks), front, heading);
        read.inline_tags = Self::inline_tags(text);
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
        "date" => front.date = at((value != "false").then(|| when(value)), span),
        "updated" => front.updated = at((value != "false").then(|| when(value)), span),
        "slug" => front.slug = at(value.parse().unwrap(), span),
        "tags" => front.tags = at(value.split(", ").map(|t| Tag::new(t).unwrap()).collect(), span),
        "publish" => front.publish = at(value == "true", span),
        "draft" => front.draft = at(value == "true", span),
        "description" => front.description = at((value != "false").then(|| value.to_owned()), span),
        "lang" => front.language = at(value.parse().unwrap(), span),
        "aliases" => front.aliases = at(value.split(", ").map(str::to_owned).collect(), span),
        "type" => front.kind = at(if value == "post" { PageKind::Post } else { PageKind::Page }, span),
        other => return Err(other.to_owned()),
    }
    Ok(())
}

fn when(text: &str) -> When {
    text.parse().map_or_else(|_| When::Instant(text.parse().unwrap()), When::Day)
}

/// A level 2 heading of text with `![[target]]` embedded in it.
fn headed(text: &str) -> Block {
    let anchor = model::Anchor::of_heading("among").unwrap();
    Block::Heading { level: Level::new(2).unwrap(), anchor, inlines: among(text) }
}

/// Text with `![[target]]` embedded in it, as the inlines it is: text, embed, text.
fn among(text: &str) -> Vec<Inline> {
    let mut inlines = Vec::new();
    let mut rest = text;
    while let Some((before, after)) = rest.split_once("![[") {
        let (written, after) = after.split_once("]]").unwrap();
        inlines.push(Inline::Text(before.to_owned()));
        inlines.push(Inline::Embed { target: LinkTarget::Unresolved { written: written.to_owned() } });
        rest = after;
    }
    inlines.push(Inline::Text(rest.to_owned()));
    inlines
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
