//! The markdown format: one file's bytes into a [`model::Read`].
//!
//! A file is UTF-8 text, optionally starting with a byte order mark, then optionally a YAML
//! frontmatter block between `---` lines, then a `CommonMark` body with these extensions: tables,
//! strikethrough (`~~x~~`), autolinks (bare `https://` and `www.` links, email addresses), alerts
//! (`> [!note]`, which become callouts), footnotes, task lists, highlight (`==x==`), superscript
//! (`^x^`), subscript (`~x~`: one tilde is subscript, not strikethrough as in GitHub's markdown)
//! and definition lists (`Term` then `: details`). `$x$` and `$$x$$` are math, as are ` ```math `
//! blocks.
//!
//! **Raw HTML** goes through the allowlist of decision 0027:
//!
//! - inline, `<kbd>` is a key, `<sup>`, `<sub>` and `<mark>` are superscript, subscript and
//!   highlight, `<del>` and `<s>` are strikethrough, `<br>` is a line break, `<img src alt>` an
//!   image and `<a href>` a link, `src` and `href` read as markdown's;
//! - as blocks, `<details>` with an optional first `<summary>` folds the markdown up to its
//!   `</details>`, `<hr>` alone is a rule and `<br>` alone a line of its own.
//!
//! Tags are matched without regard to case and paired among siblings, so `<kbd>Ctrl</kbd>` and
//! `<sup><a href="#n">1</a></sup>` nest as written. No other attribute is kept, `on*` handlers
//! included, since nothing writes raw attributes out. Every other element, and an allowed one
//! left unclosed, closed out of order or missing its `src` or `href`, is removed: its opening
//! tag (or a closing tag that closes nothing) is a `html.element.stripped` diagnostic and the
//! text inside is kept, except inside `<script>` and `<style>`, whose content is code, not
//! prose. `<ins>` and `<u>` have no node in [`model::Document`], so they are removed like any
//! other element, their text kept. An HTML comment is removed without a diagnostic: a browser
//! shows nothing for it either.
//!
//! **Wikilinks** stay text as `CommonMark` reads them; they belong to the obsidian format.
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
//! key counts as absent; nothing is guessed. A `null` value is the same as no key. `description`
//! and `summary` set the same thing: when both are set, the first written is kept and the second
//! is a `frontmatter.key.conflict` diagnostic. `lang` is a language tag (`en`, `fr-CH`).
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
