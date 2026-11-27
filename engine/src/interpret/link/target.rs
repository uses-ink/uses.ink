//! Finding the file a link names: a path relative to the page or, with a leading `/`, to the
//! root; then the same path with any extension; then a folder's index; then, anywhere under the
//! root, the files a name or a partial path names (a bare name: a file of that name or stem, or
//! a page of that alias; a path with a `/`: a file whose path ends with it, with or without its
//! extension), written case for case first and then in any case. Obsidian resolves wikilinks
//! this way, by the shortest path that is unique. More than one match is ambiguous, never a
//! guess, unless exactly one of them is in the vault's attachment folder.

use std::collections::{BTreeMap, BTreeSet};

use model::{ContentPath, Entry};

use crate::interpret::decide::Filed;
use crate::interpret::file::name::Name;
use crate::interpret::file::read::Source;
use crate::interpret::tree::{InRoot, Scope};
use crate::interpret::vault::Attachments;

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
    /// What the one of several files in the vault's attachment folder is, and every file it
    /// could name, in path order: settled, and to be said.
    Settled(Box<Found>, Vec<ContentPath>),
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
    pub(crate) aliases: Aliases,
    /// The vault's attachment folder, which settles a name several files share.
    pub(crate) attachments: Option<Attachments>,
}

/// The pages each alias names (frontmatter `aliases`), by the alias in lower case.
#[derive(Debug, Default)]
pub(crate) struct Aliases(BTreeMap<String, BTreeSet<ContentPath>>);

impl<'a> FromIterator<&'a Source> for Aliases {
    fn from_iter<I: IntoIterator<Item = &'a Source>>(sources: I) -> Self {
        let mut aliases = Self::default();
        for source in sources {
            let names = source.read.front.aliases.iter().flat_map(|aliases| &aliases.value);
            for name in names {
                aliases.0.entry(name.trim().to_lowercase()).or_default().insert(source.path.clone());
            }
        }
        aliases
    }
}

/// How a name is compared with a file's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    /// Letter for letter.
    Kept,
    /// Without regard to case, as Obsidian compares names.
    Folded,
}

impl Case {
    fn same(self, one: &str, other: &str) -> bool {
        match self {
            Self::Kept => one == other,
            Self::Folded => one.to_lowercase() == other.to_lowercase(),
        }
    }
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
        for case in [Case::Kept, Case::Folded] {
            if found.is_empty() {
                found = self.anywhere(&decoded, case);
            }
        }
        if found.len() > 1
            && let Some(attachments) = &self.attachments
        {
            let held: Vec<&ContentPath> =
                found.iter().filter(|path| attachments.holds(self.scope, from, path)).collect();
            if let [only] = held.as_slice() {
                let chosen = self.classify((*only).clone());
                return Found::Settled(Box::new(chosen), found);
            }
        }
        match found.len() {
            0 => Found::Missing,
            1 => found.pop().map_or(Found::Missing, |path| self.classify(path)),
            _ => Found::Ambiguous(found),
        }
    }

    /// Files anywhere under the root that `written` names, compared in `case`: by name, stem or
    /// alias for a bare name, by the end of their path for a path.
    fn anywhere(&self, written: &str, case: Case) -> Vec<ContentPath> {
        let written = written.trim_start_matches('/');
        if !written.contains('/') {
            let mut found = self.named(written, case);
            let aliased = self.aliases.0.iter().filter(|(alias, _)| case.same(alias, &written.to_lowercase()));
            for path in aliased.flat_map(|(_, paths)| paths) {
                if !found.contains(path) {
                    found.push(path.clone());
                }
            }
            found.sort();
            return found;
        }
        let ends = |path: &str| {
            path.len() >= written.len()
                && path.get(path.len() - written.len()..).is_some_and(|end| case.same(end, written))
                && (path.len() == written.len()
                    || path.get(..path.len() - written.len()).is_some_and(|start| start.ends_with('/')))
        };
        self.scope
            .inside
            .iter()
            .filter(|file| {
                let relative = file.relative.as_str();
                let bare = relative
                    .strip_suffix(file.relative.extension().unwrap_or_default())
                    .and_then(|bare| bare.strip_suffix('.'));
                ends(relative) || bare.is_some_and(ends)
            })
            .map(InRoot::path)
            .cloned()
            .collect()
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

    /// Files under the root whose name or stem is `name`, compared in `case`.
    fn named(&self, name: &str, case: Case) -> Vec<ContentPath> {
        self.scope
            .inside
            .iter()
            .map(Filed::path)
            .filter(|path| case.same(path.file_name(), name) || case.same(path.stem(), name))
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
