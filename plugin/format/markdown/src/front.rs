//! The YAML frontmatter block: where it is, and the keys the engine reads, each with its span.

mod error;
mod value;

use model::{Diagnostic, Document, Front, Problem, Read, Span, Subject};
use serde::Deserialize;
use serde_saphyr::Spanned as Located;

use error::Error;
use value::{Field, Raw};

/// Where the frontmatter and the body are in the file.
#[derive(Debug)]
pub(crate) struct Parts {
    pub(crate) front: Option<Block>,
    /// Where the body starts.
    pub(crate) body: usize,
}

#[derive(Debug)]
pub(crate) enum Block {
    /// The YAML between the fences, and the whole block with its fences.
    Closed { yaml: Span, block: Span },
    /// An opening `---` with no closing line: the opening line.
    Unclosed { opening: Span },
}

/// The frontmatter keys the engine reads, as written. Every other key is the author's.
#[derive(Debug, Deserialize)]
struct Keys {
    title: Option<Located<Raw>>,
    date: Option<Located<Raw>>,
    updated: Option<Located<Raw>>,
    slug: Option<Located<Raw>>,
    tags: Option<Located<Raw>>,
    publish: Option<Located<Raw>>,
    draft: Option<Located<Raw>>,
    #[serde(rename = "type")]
    kind: Option<Located<Raw>>,
    description: Option<Located<Raw>>,
    summary: Option<Located<Raw>>,
    lang: Option<Located<Raw>>,
}

/// Finds a frontmatter block at `base`: a `---` line, YAML, and a `---` or `...` line.
pub(crate) fn split(text: &str, base: usize) -> Parts {
    let mut lines = Lines { text, at: base };
    let Some(first) = lines.next() else { return Parts { front: None, body: base } };
    if !first.is("---") {
        return Parts { front: None, body: base };
    }
    let opening = first.span;
    for line in lines {
        if line.is("---") || line.is("...") {
            let yaml = Span::new(opening.end(), line.span.start());
            let block = Span::new(opening.start(), line.span.end());
            if let (Ok(yaml), Ok(block)) = (yaml, block) {
                return Parts { front: Some(Block::Closed { yaml, block }), body: line.span.end() };
            }
        }
    }
    Parts { front: Some(Block::Unclosed { opening }), body: base }
}

/// The frontmatter of `parts`, read into an otherwise empty [`Read`].
pub(crate) fn read(text: &str, parts: &Parts, subject: &Subject) -> Read {
    let mut read = Read::new(Document::default(), Front::default(), None);
    match parts.front {
        None => {}
        Some(Block::Unclosed { opening }) => {
            let problem = Problem::from(&Error::Unclosed);
            read.diagnostics.push(Diagnostic::new(problem).on(subject.clone()).at(opening));
        }
        Some(Block::Closed { yaml, block }) => {
            read.front.block = Some(block);
            let source = Source { text, yaml, subject };
            source.keys(&mut read);
        }
    }
    read
}

/// The YAML of one frontmatter block and whose it is.
#[derive(Debug)]
struct Source<'a> {
    text: &'a str,
    yaml: Span,
    subject: &'a Subject,
}

impl Source<'_> {
    fn keys(&self, read: &mut Read) {
        // `split` cuts at line ends, which are character boundaries
        let Some(yaml) = self.text.get(self.yaml.range()) else { return };
        match serde_saphyr::from_str::<Option<Keys>>(yaml) {
            Ok(Some(keys)) => self.fill(keys, read),
            Ok(None) => {}
            Err(error) => {
                let span = error.location().and_then(|at| self.span(at)).unwrap_or(self.yaml);
                // YAML counts lines from the block; the author counts them from the file
                let before = self.text.get(..self.yaml.start()).map_or(0, |before| before.matches('\n').count() as u64);
                let problem = Problem::from(&Error::Yaml { source: error, before });
                read.diagnostics.push(Diagnostic::new(problem).on(self.subject.clone()).at(span));
            }
        }
    }

    fn fill(&self, keys: Keys, read: &mut Read) {
        let mut out = value::Out { source: self, diagnostics: &mut read.diagnostics };
        let front = &mut read.front;
        front.title = keys.title.and_then(|raw| out.convert(Field::Title, &raw, value::text));
        front.date = keys.date.and_then(|raw| out.convert(Field::Date, &raw, value::when));
        front.updated = keys.updated.and_then(|raw| out.convert(Field::Updated, &raw, value::when));
        front.slug = keys.slug.and_then(|raw| out.convert(Field::Slug, &raw, value::slug));
        front.tags = keys.tags.and_then(|raw| out.tags(&raw));
        front.publish = keys.publish.and_then(|raw| out.convert(Field::Publish, &raw, value::flag));
        front.draft = keys.draft.and_then(|raw| out.convert(Field::Draft, &raw, value::flag));
        front.kind = keys.kind.and_then(|raw| out.convert(Field::Kind, &raw, value::kind));
        let description = keys.description.and_then(|raw| out.convert(Field::Description, &raw, value::text));
        let summary = keys.summary.and_then(|raw| out.convert(Field::Summary, &raw, value::text));
        front.description = out.either((Field::Description, description), (Field::Summary, summary));
        front.language = keys.lang.and_then(|raw| out.convert(Field::Language, &raw, value::language));
    }

    /// A YAML location as a span of the file.
    fn span(&self, at: serde_saphyr::Location) -> Option<Span> {
        let span = at.span();
        let start = usize::try_from(span.byte_offset()?).ok()?.checked_add(self.yaml.start())?;
        let end = start.checked_add(usize::try_from(span.byte_len()?).ok()?)?;
        Span::new(start, end.min(self.yaml.end())).ok()
    }

    /// The text written at `span`.
    fn written(&self, span: Span) -> Option<&str> {
        self.text.get(span.range())
    }
}

/// One line of the file and where it is, its line ending included in the span.
#[derive(Debug)]
struct Line<'a> {
    text: &'a str,
    span: Span,
}

impl Line<'_> {
    /// Whether the line is `fence`, with nothing after it but spaces, tabs and the line ending.
    fn is(&self, fence: &str) -> bool {
        self.text.trim_end_matches(['\n', '\r', ' ', '\t']) == fence
    }
}

#[derive(Debug)]
struct Lines<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Iterator for Lines<'a> {
    type Item = Line<'a>;

    fn next(&mut self) -> Option<Line<'a>> {
        let rest = self.text.get(self.at..).filter(|rest| !rest.is_empty())?;
        let len = rest.find('\n').map_or(rest.len(), |end| end + 1);
        let text = rest.get(..len)?;
        let span = Span::new(self.at, self.at + len).ok()?;
        self.at += len;
        Some(Line { text, span })
    }
}
