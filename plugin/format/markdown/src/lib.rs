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
//! **Frontmatter** is read through the model: each key the engine reads belongs to a decision
//! code ([`model::code::Keyed`]) or is `draft` or `lang`, and its value is read by the value
//! type's [`model::FromFront`], so this crate says what YAML is and nothing about what a date
//! or a tag is. Keys the engine does not read are the author's own and are ignored. A key it
//! reads with a value it cannot use is a `frontmatter.value.invalid` diagnostic for each refused
//! part, in the order written, and the key counts as absent; nothing is guessed. A list is
//! refused whole when any item is (`tags: [good, "!!!"]` sets no tags), with a diagnostic at
//! every bad item. A `null` value is the same as no key. `false` for a key whose value may be
//! none (`date: false`) is refused until the frontmatter can hold an explicit none. Keys that set
//! the same thing (`description` and `summary`): when several are set, the first written is kept
//! and each later one is a `frontmatter.key.conflict` diagnostic. `lang` is a language tag
//! (`en`, `fr-CH`). A key written twice is a `frontmatter.syntax.invalid` diagnostic.
//!
//! Until `host/wasmtime` exists this crate speaks `uses-ink-model` directly; it moves to the
//! generated WIT types when it becomes a component.

/// comrak, behind model types: nothing else in the crate names it.
mod body;
mod error;
mod file;
mod front;
mod html;

use model::{ContentPath, Document, Front, Read, Subject};

use error::Report;
use file::File;
use front::{Keys, Yaml};

/// Reads one markdown file. Pure: the same bytes give the same [`Read`], and no input panics.
pub fn read(path: &ContentPath, bytes: &[u8]) -> Read {
    let subject = Subject::File(path.clone());
    let mut report = Report::new(subject.clone());
    let mut read = match File::try_from(bytes) {
        Ok(file) => {
            let front = Yaml::read::<Keys>(&file, &mut report);
            let body = body::Body::read(file.text, file.body, subject);
            report.diagnostics.extend(body.diagnostics);
            report.notices.extend(body.notices);
            Read::new(body.document, front, body.heading)
        }
        Err(encoding) => {
            let span = encoding.span();
            report.refuse(encoding.into(), span);
            Read::new(Document::default(), Front::default(), None)
        }
    };
    read.diagnostics = report.diagnostics;
    read.notices = report.notices;
    read
}
