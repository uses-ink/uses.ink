//! Finding the file a link names: a path relative to the page or, with a leading `/`, to the
//! root; then the same path with any extension; then a folder's index; then, for a bare name, a
//! unique file name anywhere under the root. More than one match is ambiguous, never a guess.

use std::collections::BTreeSet;

use model::{ContentPath, Entry};

use crate::interpret::file::name::Name;
use crate::interpret::file::scope::{InRoot, Scope};

/// What a link's path names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Found {
    /// A published page.
    Page(ContentPath),
    /// A kept file no format reads.
    File(ContentPath),
    /// A page that is not published, or a file that is ignored or outside the root.
    Unpublished(ContentPath),
    Missing,
    /// Every file it could name, in path order.
    Ambiguous(Vec<ContentPath>),
}

/// Everything a link can point at.
#[derive(Debug)]
pub(crate) struct Targets<'a> {
    pub(crate) tree: &'a [Entry],
    pub(crate) scope: &'a Scope<'a>,
    /// Files under the root that are not ignored.
    pub(crate) kept: BTreeSet<ContentPath>,
    /// Files a format read.
    pub(crate) pages: BTreeSet<ContentPath>,
    pub(crate) published: BTreeSet<ContentPath>,
}

impl Targets<'_> {
    /// The file `written`, a link's path without its anchor, names from the page at `from`.
    pub(crate) fn find(&self, from: &ContentPath, written: &str) -> Found {
        let decoded = decode(written);
        let Some(target) = self.join(from, &decoded) else { return Found::Missing };
        if self.holds(&target) {
            return self.classify(target);
        }
        let mut found: Vec<ContentPath> = self
            .tree
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
            found = self.by_name(&decoded);
        }
        match found.len() {
            0 => Found::Missing,
            1 => found.pop().map_or(Found::Missing, |path| self.classify(path)),
            _ => Found::Ambiguous(found),
        }
    }

    /// `written` against the page's folder, or against the root after a leading `/`.
    fn join(&self, from: &ContentPath, written: &str) -> Option<ContentPath> {
        let base = match written.strip_prefix('/') {
            Some(_) => self.scope.absolute_folder(),
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
        self.tree.binary_search_by(|entry| entry.path.cmp(path)).is_ok()
    }

    /// Files under the root whose name or stem is `name`.
    fn by_name(&self, name: &str) -> Vec<ContentPath> {
        self.scope
            .inside
            .iter()
            .map(InRoot::path)
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

/// `%XX` escapes decoded, as links to names with spaces are written. Text whose escapes do not
/// decode to UTF-8 is taken as written.
fn decode(written: &str) -> String {
    let bytes = written.as_bytes();
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
    String::from_utf8(out).unwrap_or_else(|_| written.to_owned())
}
