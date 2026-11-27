//! The obsidian format: one note of an Obsidian vault into a [`model::Read`].
//!
//! A note is markdown as `uses-ink-format-markdown` reads it, through the same comrak setup
//! and the same raw HTML allowlist (decision 0027), in the dialect [`Obsidian`]: what Obsidian
//! adds, and without what it lacks: `^x^` superscript and definition lists stay text, as
//! Obsidian shows them, and so does `~x~`: only `~~x~~` is struck through (comrak strikes both, so
//! the dialect turns a single-tilde pair back into the text it was, outside code).
//!
//! A note's title is its file name, as Obsidian shows it: its first heading is a heading, and the
//! frontmatter `title`, when there is one, still comes first.
//!
//! - **Wikilinks**: `[[Note]]`, `[[Note|text]]`, `[[Note#Heading]]`, `[[Note#^block]]` and
//!   `[[#Heading]]` are links, left unresolved for the engine with the target as written, the
//!   anchor being the last `#` part (`[[Note#A#B]]` names `B`). Without a `|`, the text is the
//!   target as Obsidian shows it: `Note > Heading`, and no `.md`.
//! - **Embeds**: `![[image.png]]` is an image (its alt text the text after the `|`, or the file
//!   name); a size after the `|` (`|300`, `|300x200`) is not kept and is said
//!   (`embed.size.ignored`). `![[Note]]`, `![[Note#Heading]]` and `![[Note#^block]]` alone in
//!   their paragraph are a [`model::Block::Embed`] the engine fills with the page or its part;
//!   among other text they are a [`model::Inline::Embed`], which the engine makes a block between
//!   the text before and the text after. An embed of another kind of file
//!   (a PDF, audio, video, a canvas) is a link (`embed.file.linked`).
//! - **Callouts**: `> [!kind] Title`, any kind, the title of inlines, `+` or `-` after the kind
//!   for a callout that folds open or closed. Obsidian's aliases name its kinds (`hint` is a tip,
//!   `attention` a warning, `tldr` an `abstract`); a kind it does not know keeps its own name.
//!   Metadata after a `|` is not kept (`callout.metadata.ignored`).
//! - **Block ids**: `^id` ending a block, or alone on the line after one, is an anchor; it
//!   shares the page's anchors with its headings, and an id whose anchor is taken is
//!   `block.id.taken`.
//! - **Comments**: `%%text%%` and `%%` blocks are removed, as HTML comments are.
//! - **Tags**: `#tag` and nested `#a/b` in the text, after a space or at the start of a run of
//!   text and holding something other than digits, join the frontmatter's `tags` in the order
//!   written, each once by its slug; they stay in the text as written.
//! - **Aliases**: frontmatter `aliases` (or `alias`), a list or one name, are the other names a
//!   link may use for the note; the engine resolves `[[Alias]]` through them.
//!
//! Nothing here resolves a link or reads another file: the vault's settings
//! (`.obsidian/app.json`) and the other notes are the engine's. This crate is the format's
//! native core; `guest/` exports it as the `ink:format` world of a wasm component.

mod callout;
mod link;
mod tag;

use format_markdown::dialect::{Callouts, Dialect, Extension, Linked, Syntax, Titled, Wikilink};
use model::{CalloutKind, ContentPath, Read, Span};

use link::Target;
use tag::Tags;

/// Whether a file is a note: a markdown file, as `uses-ink-format-markdown` says.
pub fn handles(path: &ContentPath) -> bool {
    format_markdown::handles(path)
}

/// Reads one note. Pure: the same bytes give the same [`Read`], and no input panics.
pub fn read(path: &ContentPath, bytes: &[u8]) -> Read {
    format_markdown::read_as(Obsidian::default(), path, bytes)
}

/// Obsidian's markdown, as a dialect: one value reads one note, gathering its tags.
#[derive(Debug, Default)]
pub struct Obsidian {
    tags: Tags,
}

impl Dialect for Obsidian {
    const SYNTAX: Syntax = Syntax {
        callouts: Callouts::Folding,
        title: Titled::File,
        extensions: &[
            Extension::Wikilinks,
            Extension::BlockIds,
            Extension::Comments,
            Extension::Aliases,
            Extension::DoubleStrike,
        ],
    };

    fn wikilink(&mut self, link: Wikilink<'_>) -> Linked {
        Target::from(link.target).linked(link)
    }

    fn callout(&self, kind: &str) -> Option<CalloutKind> {
        callout::Kind::from(kind).into()
    }

    fn text(&mut self, text: &str, span: Span) {
        self.tags.scan(text, span);
    }

    fn finish(self, read: &mut Read) {
        self.tags.join(&mut read.front);
    }
}
