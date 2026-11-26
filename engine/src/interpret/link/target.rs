//! Finding the file a link names: a path relative to the page or, with a leading `/`, to the
//! root; then the same path with any extension; then a folder's index; then, for a bare name, a
//! unique file name anywhere under the root. More than one match is ambiguous, never a guess.

use std::collections::BTreeSet;

use model::{ContentPath, Entry};

use crate::interpret::decide::Filed;
use crate::interpret::file::name::Name;
use crate::interpret::tree::Scope;

/// A link's target as the author wrote it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Written<'a>(pub(crate) &'a str);

impl<'a> Written<'a> {
    /// The path and the anchor after a `#`, if there is one.
    pub(crate) fn split(self) -> (&'a str, Option<&'a str>) {
        self.0.split_once('#').map_or((self.0, None), |(path, anchor)| (path, Some(anchor)))
    }

    /// Whether it starts with a URL scheme (`https:`, `mailto:`, `javascript:`), so it is never
    /// a path.
    pub(crate) fn is_external(self) -> bool {
        self.0.split_once(':').is_some_and(|(scheme, _)| {
            let mut chars = scheme.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        })
    }

    /// `%XX` escapes decoded, as links to names with spaces are written. Text whose escapes do
    /// not decode to UTF-8 is taken as written.
    fn decoded(self) -> String {
        let bytes = self.0.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        while let Some(&byte) = bytes.get(at) {
            let escape = bytes
                .get(at + 1..at + 3)
                .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
                .and_then(|hex| std::str::from_utf8(hex).ok());
            match escape.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                Some(decoded) if byte == b'%' => {
                    out.push(decoded);
                    at += 3;
                }
                _ => {
                    out.push(byte);
                    at += 1;
                }
            }
        }
        String::from_utf8(out).unwrap_or_else(|_| self.0.to_owned())
    }
}

impl From<Written<'_>> for String {
    fn from(written: Written<'_>) -> Self {
        written.0.to_owned()
    }
}

/// What a link's path names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Found {
    /// A published page.
    Page(ContentPath),
    /// A kept file no format reads.
    File(ContentPath),
    /// A page that is not published, or a file that is ignored or outside the root.
    Unpublished(ContentPath),
    /// Nothing.
    Missing,
    /// Every file it could name, in path order.
    Ambiguous(Vec<ContentPath>),
}

/// Everything a link can point at.
#[derive(Debug)]
pub(crate) struct Targets<'a> {
    pub(crate) scope: &'a Scope<'a>,
    /// Files under the root that are not ignored.
    pub(crate) kept: BTreeSet<ContentPath>,
    /// Files a format read.
    pub(crate) pages: BTreeSet<ContentPath>,
    pub(crate) published: BTreeSet<ContentPath>,
}

impl Targets<'_> {
    /// The file `written`, a link's path without its anchor, names from the page at `from`.
    pub(crate) fn find(&self, from: &ContentPath, written: Written<'_>) -> Found {
        let decoded = written.decoded();
        let Some(target) = self.join(from, &decoded) else { return Found::Missing };
        if self.holds(&target) {
            return self.classify(target);
        }
        let mut found: Vec<ContentPath> = self
            .tree()
            .iter()
            .map(|entry| &entry.path)
            .filter(|path| {
                let sibling = path.parent() == target.parent() && path.extension().is_some();
                let index = path.parent().as_ref() == Some(&target) && Name::from(*path).is_index();
                (sibling && path.stem() == target.file_name()) || index
            })
            .cloned()
            .collect();
        if found.is_empty() && !decoded.contains('/') {
            found = self.named(&decoded);
        }
        match found.len() {
            0 => Found::Missing,
            1 => found.pop().map_or(Found::Missing, |path| self.classify(path)),
            _ => Found::Ambiguous(found),
        }
    }

    fn tree(&self) -> &[Entry] {
        self.scope.snapshot.tree()
    }

    /// `written` against the page's folder, or against the root after a leading `/`.
    fn join(&self, from: &ContentPath, written: &str) -> Option<ContentPath> {
        let base = match written.strip_prefix('/') {
            Some(_) => self.scope.folder().cloned(),
            None => from.parent(),
        };
        let mut segments: Vec<&str> = base.as_ref().map_or_else(Vec::new, |base| base.segments().collect());
        for part in written.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    segments.pop()?;
                }
                part => segments.push(part),
            }
        }
        ContentPath::parse(segments.join("/")).ok()
    }

    fn holds(&self, path: &ContentPath) -> bool {
        self.tree().binary_search_by(|entry| entry.path.cmp(path)).is_ok()
    }

    /// Files under the root whose name or stem is `name`.
    fn named(&self, name: &str) -> Vec<ContentPath> {
        self.scope
            .inside
            .iter()
            .map(Filed::path)
            .filter(|path| path.file_name() == name || path.stem() == name)
            .cloned()
            .collect()
    }

    fn classify(&self, path: ContentPath) -> Found {
        if self.published.contains(&path) {
            Found::Page(path)
        } else if self.pages.contains(&path) || !self.kept.contains(&path) {
            Found::Unpublished(path)
        } else {
            Found::File(path)
        }
    }
}
