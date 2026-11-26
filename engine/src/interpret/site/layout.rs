//! `site.layout`: how the site's files are arranged, which sets later defaults.

use model::{ContentPath, DefaultReason, Evidence, Layout, Root, Subject};

use crate::interpret::file::name::Name;
use crate::interpret::file::scope::Scope;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::page::kind::POSTS;
use crate::interpret::record::Record;
use crate::pins::Pins;

/// The folder Obsidian keeps a vault's settings in.
const VAULT: &str = ".obsidian";

/// The root folder that makes a site docs.
const DOCS: &str = "docs";

/// Pin, then `.obsidian/` (vault), then a `docs` root (docs), then a top folder of posts or a
/// dated file name (blog), then flat.
pub(crate) fn decide(scope: &Scope<'_>, pins: &impl Pins, record: &mut Record) -> Layout {
    let marker = |path: &ContentPath| Evidence::Marker { path: path.clone() };
    let vault = scope
        .inside
        .iter()
        .find(|file| file.relative.segments().next() == Some(VAULT))
        .map(|file| (Layout::Vault, marker(file.path())));
    let docs = match &scope.root {
        Root::Folder(folder) if folder.as_str() == DOCS => Some((Layout::Docs, marker(folder))),
        _ => None,
    };
    let posts = scope.inside.iter().find_map(|file| {
        let top = file.relative.segments().next().filter(|top| POSTS.contains(top) && file.relative.parent().is_some());
        let folder = ContentPath::parse(top?).ok()?;
        Some((Layout::Blog, marker(&scope.absolute(&folder))))
    });
    let dated = scope
        .inside
        .iter()
        .find(|file| Name::from(&file.relative).date().is_some())
        .map(|file| (Layout::Blog, Evidence::Filename { path: file.path().clone() }));
    Ladder::<code::Layout>::new(Subject::Site, pins, record).rung(vault).rung(docs).rung(posts).rung(dated).or_default(
        Layout::Flat,
        DefaultReason::NothingMatched,
        record,
    )
}
