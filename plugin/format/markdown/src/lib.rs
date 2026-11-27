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
//! none (`date`, `updated`, `description`) is an explicit none, held as `Some(None)`: the page
//! is decided undated, never updated or without a summary, not left to the file name or the
//! text. Keys that set
//! the same thing (`description` and `summary`): when several are set, the first written is kept
//! and each later one is a `frontmatter.key.conflict` diagnostic. `lang` is a language tag
//! (`en`, `fr-CH`). A key written twice is a `frontmatter.syntax.invalid` diagnostic.
//!
//! **Dialects.** [`read`] reads the markdown above. [`read_as`] reads a file in another dialect
//! of it, such as Obsidian's, through the same comrak setup: a [`dialect::Dialect`] turns on
//! the syntax it adds and says what its own nodes mean, so a dialect never sees comrak.
//!
//! This crate is the format's native core and speaks `uses-ink-model` directly; `guest/` exports
//! it as the `ink:format` world of a wasm component (decision 0035).

mod body;
pub mod dialect;
mod error;
mod file;
mod front;
mod html;

use model::{ContentPath, Document, Front, Read, Subject};

use body::Builder;
use dialect::{Dialect, Extension, Markdown};
use error::Report;
use file::File;
use front::{Aliased, Keys, Yaml};

pub use front::{Edit, Malformed, Publishing, Refusal};

/// Whether a file is markdown: its extension is `md` or `markdown`, in any case.
pub fn handles(path: &ContentPath) -> bool {
    path.extension().is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"))
}

/// Reads one markdown file. Pure: the same bytes give the same [`Read`], and no input panics.
pub fn read(path: &ContentPath, bytes: &[u8]) -> Read {
    read_as(Markdown, path, bytes)
}

/// Reads one file in `dialect`. Pure, as [`read`].
pub fn read_as<D: Dialect>(mut dialect: D, path: &ContentPath, bytes: &[u8]) -> Read {
    let mut report = Report::new(Subject::File(path.clone()));
    let mut read = match File::try_from(bytes) {
        Ok(file) => {
            let front = if D::SYNTAX.has(Extension::Aliases) {
                Yaml::read::<Aliased>(&file, &mut report)
            } else {
                Yaml::read::<Keys>(&file, &mut report)
            };
            let body = Builder::read(&file, &mut report, &mut dialect);
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
    dialect.finish(&mut read);
    read
}
