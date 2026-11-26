//! The markdown format: one file's bytes into a [`model::Read`].
//!
//! A file is UTF-8 text, optionally starting with a byte order mark, then optionally a YAML
//! frontmatter block between `---` lines, then a `CommonMark` body with these GitHub extensions:
//! tables, strikethrough, autolinks (bare `https://` and `www.` links, email addresses) and alerts
//! (`> [!note]`, which become callouts). `$x$` and `$$x$$` are math, as are ` ```math ` blocks.
//!
//! What it does not read, and why:
//!
//! - **Raw HTML** is removed, block and inline alike. Every opening tag is a
//!   `html.element.stripped` diagnostic at that tag; a closing tag is part of the element
//!   already reported. Text between tags is kept as text, except inside `<script>` and `<style>`,
//!   whose content is code, not prose. An HTML comment is removed without a diagnostic: a browser
//!   shows nothing for it either, so nothing a reader sees changes. The allowlist of decision
//!   0027, which keeps safe elements as `Document` nodes, comes later.
//! - **Footnotes**, task lists and wikilinks have no node in [`model::Document`] yet, so their
//!   syntax stays text as `CommonMark` reads it. Wikilinks belong to the obsidian format.
//!
//! Nothing here resolves a link. `http(s):` and `mailto:` links become
//! [`model::LinkTarget::External`], `#anchor` alone becomes [`model::LinkTarget::Anchor`] when
//! it is written as a slug, and everything else stays [`model::LinkTarget::Unresolved`] with the
//! text as written, for the engine to resolve once it knows every page. Images likewise.
//!
//! Every heading gets an anchor: the slug of its text, then `-1`, `-2` in document order when
//! taken. A heading with no letters or digits is anchored `section` with a
//! `heading.anchor.generated` notice.
//!
//! Frontmatter keys the engine does not read are the author's own and are ignored. A key it reads
//! with a value it cannot use is a `frontmatter.value.invalid` diagnostic at that value, and the
//! key counts as absent; nothing is guessed. A `null` value is the same as no key.
//!
//! Until `host/wasmtime` exists this crate speaks `uses-ink-model` directly; it moves to the
//! generated WIT types when it becomes a component.

/// comrak, behind model types: nothing else in the crate names it.
mod body;
/// serde-saphyr, behind model types: nothing else in the crate names it.
mod front;
mod html;

use model::{ContentPath, Diagnostic, Document, Front, Problem, Read, Span, Subject};

const BYTE_ORDER_MARK: &str = "\u{feff}";

/// Reads one markdown file. Pure: the same bytes give the same [`Read`], and no input panics.
pub fn read(path: &ContentPath, bytes: &[u8]) -> Read {
    let subject = Subject::File(path.clone());
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => return unreadable(subject, error.valid_up_to(), bytes.len()),
    };
    let base = if text.starts_with(BYTE_ORDER_MARK) { BYTE_ORDER_MARK.len() } else { 0 };
    let parts = front::split(text, base);
    let mut read = front::read(text, &parts, &subject);
    let body = body::Body::read(text, parts.body, subject);
    read.document = body.document;
    read.heading = body.heading;
    read.diagnostics.extend(body.diagnostics);
    read.notices.extend(body.notices);
    read
}

/// The read of a file that is not UTF-8: empty, with the offset of the first bad byte.
fn unreadable(subject: Subject, at: usize, len: usize) -> Read {
    let mut read = Read::new(Document::default(), Front::default(), None);
    let mut diagnostic = Diagnostic::new(Problem::SourceFileEncoding { at: at as u64 }).on(subject);
    if let Ok(span) = Span::new(at, len.min(at + 1)) {
        diagnostic = diagnostic.at(span);
    }
    read.diagnostics.push(diagnostic);
    read
}
