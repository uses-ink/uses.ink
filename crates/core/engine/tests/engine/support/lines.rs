//! A stand-in format that reads a few lines.

use engine::Format;
use model::{
    Block, ContentPath, Diagnostic, Document, Front, ImageSource, Inline, Level, LinkTarget, PageKind, Problem, Read,
    Span, Spanned, Tag, When,
};

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
