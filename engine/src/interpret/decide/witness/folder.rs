//! [`Folder`]: what a folder says by being there, named by a marker `M`: a posts folder, a
//! drafts folder, a hidden or vendored folder, a vault's `.obsidian/`, a docs root, the one
//! content folder.

use std::marker::PhantomData;

use model::code::{Code, Ignored, Kind, Layout, Publish, Root, SiteTitle};
use model::{ContentPath, Evidence, PageKind};

use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};
use crate::interpret::site;
use crate::interpret::tree::{Scope, Tree};

/// A folder marker that says a code's value of a file it holds.
pub(crate) trait Mark<C: Code> {
    /// The value for a file at `relative`, a path below the root, if the marker holds it.
    fn mark(relative: &ContentPath) -> Option<C::Value>;
}

/// The folder marked `M`. On a file's ground, a file it holds, evidence `Filename`; on the
/// site's, the folder itself, evidence `Marker`.
#[derive(Debug, Default)]
pub(crate) struct Folder<M>(PhantomData<M>);

impl<C: Code, M: Mark<C>, G: Filed + ?Sized> Witness<C, G> for Folder<M> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        if let Some(value) = M::mark(case.ground.relative()) {
            testimony.value(value, case.ground.named());
        }
    }
}

/// A top folder of posts: `posts/`, `blog/`, `writing/`.
#[derive(Debug, Default)]
pub(crate) struct Posts;

impl Posts {
    const NAMES: &[&str] = &["posts", "blog", "writing"];

    /// The posts folder holding `relative`, if one does.
    fn top(relative: &ContentPath) -> Option<&str> {
        relative.parent()?;
        relative.segments().next().filter(|top| Self::NAMES.contains(top))
    }
}

impl Mark<Kind> for Posts {
    fn mark(relative: &ContentPath) -> Option<PageKind> {
        Self::top(relative).map(|_| PageKind::Post)
    }
}

impl Witness<Layout, Scope<'_>> for Folder<Posts> {
    /// A posts folder under the root makes a blog.
    fn testify(&self, case: &Case<'_, Layout, Scope<'_>>, testimony: &mut Testimony<model::Layout>) {
        let scope = case.ground;
        let folder = scope.inside.iter().find_map(|file| ContentPath::parse(Posts::top(&file.relative)?).ok());
        if let Some(folder) = folder {
            testimony.value(model::Layout::Blog, Evidence::Marker { path: scope.absolute(&folder) });
        }
    }
}

/// Unfinished pages: a `drafts/` folder anywhere, or a name starting with `_`.
#[derive(Debug, Default)]
pub(crate) struct Drafts;

impl Mark<Publish> for Drafts {
    fn mark(relative: &ContentPath) -> Option<bool> {
        let drafted = relative.segments().any(|segment| segment == "drafts") || relative.file_name().starts_with('_');
        drafted.then_some(false)
    }
}

/// Dotfiles and dot folders, and folders of other tools' dependencies.
#[derive(Debug, Default)]
pub(crate) struct Hidden;

impl Mark<Ignored> for Hidden {
    fn mark(relative: &ContentPath) -> Option<bool> {
        const VENDORED: &[&str] = &["node_modules"];
        let hidden = relative.segments().any(|segment| segment.starts_with('.') || VENDORED.contains(&segment));
        hidden.then_some(true)
    }
}

/// `.obsidian/`, where Obsidian keeps a vault's settings.
#[derive(Debug, Default)]
pub(crate) struct Obsidian;

impl Witness<Layout, Scope<'_>> for Folder<Obsidian> {
    fn testify(&self, case: &Case<'_, Layout, Scope<'_>>, testimony: &mut Testimony<model::Layout>) {
        let found = case.ground.inside.iter().find(|file| file.relative.segments().next() == Some(".obsidian"));
        if let Some(file) = found {
            testimony.value(model::Layout::Vault, Evidence::Marker { path: file.entry.path.clone() });
        }
    }
}

/// A root folder named `docs`.
#[derive(Debug, Default)]
pub(crate) struct Docs;

impl Witness<Layout, Scope<'_>> for Folder<Docs> {
    fn testify(&self, case: &Case<'_, Layout, Scope<'_>>, testimony: &mut Testimony<model::Layout>) {
        if let model::Root::Folder(folder) = &case.ground.root
            && folder.as_str() == "docs"
        {
            testimony.value(model::Layout::Docs, Evidence::Marker { path: folder.clone() });
        }
    }
}

/// The folder a site's content is in.
#[derive(Debug, Default)]
pub(crate) struct Content;

impl Witness<Root, Tree<'_>> for Folder<Content> {
    /// The one folder of [`Tree::CONTENT`] present, when every file a format reads is in it.
    fn testify(&self, case: &Case<'_, Root, Tree<'_>>, testimony: &mut Testimony<model::Root>) {
        if let Some(folder) = case.ground.content() {
            testimony.value(model::Root::Folder(folder.clone()), Evidence::Marker { path: folder });
        }
    }
}

impl Witness<SiteTitle, site::Ground<'_>> for Folder<Content> {
    /// The root folder's name.
    fn testify(&self, case: &Case<'_, SiteTitle, site::Ground<'_>>, testimony: &mut Testimony<String>) {
        if let model::Root::Folder(folder) = &case.ground.scope.root {
            testimony.value(folder.file_name().to_owned(), Evidence::Marker { path: folder.clone() });
        }
    }
}
