//! [`Folder`]: what a folder says by being there, named by a marker `M`: a posts folder, a
//! drafts folder, a hidden or vendored folder, a vault's `.obsidian/`, a docs root, the one
//! content folder. Its evidence names the folder.

use std::marker::PhantomData;

use model::code::{Code, Format, Ignored, Kind, Layout, Publish, Root, SiteTitle};
use model::record::PluginId;
use model::{ContentPath, Evidence, PageKind, Snapshot};

use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};
use crate::interpret::decide::trace::{Marker, Named, Witnessed};
use crate::interpret::pipeline::file::ignore;
use crate::interpret::pipeline::file::name::Name;
use crate::interpret::pipeline::site;
use crate::interpret::pipeline::tree::{Scope, Tree};

/// What a marker found in a file's path below the root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Marked {
    /// The folder made of the path's first segments, this many.
    Folder(usize),
    /// The file's own name, such as a dotfile or a draft's leading `_`.
    Name,
}

impl Marked {
    /// The evidence of the mark on `file`: the folder in the snapshot, or the file's name.
    fn evidence<G: Filed + ?Sized>(self, file: &G) -> Evidence {
        let Self::Folder(depth) = self else { return file.named() };
        let up = file.relative().segments().count().saturating_sub(depth);
        match (0..up).try_fold(file.path().clone(), |path, _| path.parent()) {
            Some(path) => Evidence::Marker { path },
            None => file.named(),
        }
    }

    /// The first segment of `relative` matching `marks`: a folder holding the file, or the file's
    /// own name when the match is its last segment.
    fn find(relative: &ContentPath, marks: impl Fn(&str) -> bool) -> Option<Self> {
        let count = relative.segments().count();
        let at = relative.segments().position(marks)?;
        Some(if at + 1 == count { Self::Name } else { Self::Folder(at + 1) })
    }
}

/// A folder marker that says a code's value of a file it holds.
pub(crate) trait Mark<C: Code> {
    /// The value for a file at `relative`, a path below the root, and where the marker found
    /// it, if the marker holds it.
    fn mark(relative: &ContentPath) -> Option<(C::Value, Marked)>;
}

/// The folder marked `M`. On a file's ground, the folder holding the file (or the file's name,
/// for a marker a name can carry); on the site's, the folder itself. Evidence `Marker`.
#[derive(Debug, Default)]
pub(crate) struct Folder<M>(PhantomData<M>);

impl<C: Code, M: Mark<C>, G: Filed + ?Sized> Witness<C, G> for Folder<M> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        if let Some((value, marked)) = M::mark(case.ground.relative()) {
            testimony.value(value, marked.evidence(case.ground));
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
    /// A post, except the folder's own index (`posts/index.md`, `posts/README.md`), which lists
    /// the posts: a page.
    fn mark(relative: &ContentPath) -> Option<(PageKind, Marked)> {
        Self::top(relative)?;
        let listing = relative.segments().count() == 2 && Name::from(relative).is_index();
        Some((if listing { PageKind::Page } else { PageKind::Post }, Marked::Folder(1)))
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
    fn mark(relative: &ContentPath) -> Option<(bool, Marked)> {
        let folders = relative.segments().count().saturating_sub(1);
        let folder = relative.segments().take(folders).position(|segment| segment == "drafts");
        let marked = match folder {
            Some(at) => Marked::Folder(at + 1),
            None => relative.file_name().starts_with('_').then_some(Marked::Name)?,
        };
        Some((false, marked))
    }
}

/// Dotfiles and dot folders, and folders of other tools' dependencies.
#[derive(Debug, Default)]
pub(crate) struct Hidden;

impl Mark<Ignored> for Hidden {
    fn mark(relative: &ContentPath) -> Option<(bool, Marked)> {
        const VENDORED: &[&str] = &["node_modules"];
        let marked = Marked::find(relative, |segment| segment.starts_with('.') || VENDORED.contains(&segment))?;
        Some((true, marked))
    }
}

/// A vault's templates folder: the one `.obsidian/templates.json` names, else the conventional
/// `templates/` at the top of the root, in any case. Templates are never pages.
#[derive(Debug, Default)]
pub(crate) struct Templates;

impl Witness<Ignored, ignore::Ground<'_>> for Folder<Templates> {
    fn testify(&self, case: &Case<'_, Ignored, ignore::Ground<'_>>, testimony: &mut Testimony<bool>) {
        let file = case.ground;
        if file.layout != Some(model::Layout::Vault) {
            return;
        }
        if let Some(depth) = file.templates.holds(file.relative()) {
            testimony.value(true, Marked::Folder(depth).evidence(file));
        }
    }
}

/// `.obsidian/`, where Obsidian keeps a vault's settings.
#[derive(Debug, Default)]
pub(crate) struct Obsidian;

impl Folder<Obsidian> {
    /// The folder's name.
    const NAME: &str = ".obsidian";
}

impl Witness<Layout, Scope<'_>> for Folder<Obsidian> {
    fn testify(&self, case: &Case<'_, Layout, Scope<'_>>, testimony: &mut Testimony<model::Layout>) {
        let scope = case.ground;
        let found = scope.inside.iter().find(|file| file.relative.segments().next() == Some(Self::NAME));
        if let Some(folder) = found.and_then(|_| ContentPath::parse(Self::NAME).ok()) {
            testimony.value(model::Layout::Vault, Evidence::Marker { path: scope.absolute(&folder) });
        }
    }
}

impl Witness<Format, Snapshot> for Folder<Obsidian> {
    /// A folder holding `.obsidian/` at its top is a vault, as `.git` makes it a repository.
    fn testify(&self, case: &Case<'_, Format, Snapshot>, testimony: &mut Testimony<PluginId>) {
        let vault = case.ground.tree().iter().any(|entry| entry.path.segments().next() == Some(Self::NAME));
        if let Some(folder) = ContentPath::parse(Self::NAME).ok().filter(|_| vault) {
            testimony.value(PluginId::obsidian(), Evidence::Marker { path: folder });
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

impl Named for Folder<Posts> {
    const NAME: Witnessed = Witnessed::Folder(Marker::Posts);
}

impl Named for Folder<Drafts> {
    const NAME: Witnessed = Witnessed::Folder(Marker::Drafts);
}

impl Named for Folder<Hidden> {
    const NAME: Witnessed = Witnessed::Folder(Marker::Hidden);
}

impl Named for Folder<Templates> {
    const NAME: Witnessed = Witnessed::Folder(Marker::Templates);
}

impl Named for Folder<Obsidian> {
    const NAME: Witnessed = Witnessed::Folder(Marker::Obsidian);
}

impl Named for Folder<Docs> {
    const NAME: Witnessed = Witnessed::Folder(Marker::Docs);
}

impl Named for Folder<Content> {
    const NAME: Witnessed = Witnessed::Folder(Marker::Content);
}
