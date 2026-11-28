//! The site's contents as a tree of folders: what `ink.toml` says of each folder
//! ([`Section`]), the pages in reading order ([`Branch`]) and the page before and after each
//! ([`Neighbours`]). All of it is a function of the [`Site`], so a theme in a component and one
//! compiled in write the same navigation.

use std::cmp::Ordering;

use serde::Serialize;

use crate::{ContentPath, Page, PageKind, Root, Site};

/// How the pages of a folder are listed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Order {
    /// By route, which is by file name.
    #[default]
    Route,
    /// By title.
    Title,
    /// Newest first, undated pages last.
    Newest,
    /// Oldest first, undated pages last.
    Oldest,
}

/// What `ink.toml` says of one folder: where it stands among its siblings and how its pages are
/// listed (`[folder."docs/decisions"]`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Section {
    /// The folder, as a path in the snapshot.
    pub folder: ContentPath,
    /// Where it stands among the folders beside it: lower first, then by name.
    pub weight: i32,
    /// How its pages are listed.
    pub order: Order,
}

impl Section {
    /// The folder `folder`, at `weight`, listing its pages in `order`.
    pub const fn new(folder: ContentPath, weight: i32, order: Order) -> Self {
        Self { folder, weight, order }
    }
}

/// One folder of the contents: its own page (`index` or `README`), the pages in it and the
/// folders in it, each in the order the site lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Branch<'a> {
    /// The folder as a path in the snapshot; `None` for the top of the site.
    pub folder: Option<ContentPath>,
    /// The page that stands for the folder, when it has one.
    pub index: Option<&'a Page>,
    /// The other pages directly in the folder.
    pub pages: Vec<&'a Page>,
    /// The folders directly in it.
    pub branches: Vec<Branch<'a>>,
}

/// The pages before and after one page in reading order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Neighbours<'a> {
    /// The page before, if the page is not the first.
    pub previous: Option<&'a Page>,
    /// The page after, if the page is not the last.
    pub next: Option<&'a Page>,
}

impl Order {
    /// Sorts `pages`, which come in route order, which `Route` keeps.
    fn sort(self, pages: &mut [&Page]) {
        match self {
            Self::Route => {}
            Self::Title => pages.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.route.cmp(&b.route))),
            Self::Newest => pages.sort_by(|a, b| match (a.date, b.date) {
                (Some(x), Some(y)) => y.cmp(&x).then_with(|| a.route.cmp(&b.route)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => a.route.cmp(&b.route),
            }),
            Self::Oldest => pages.sort_by(|a, b| match (a.date, b.date) {
                (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.route.cmp(&b.route)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => a.route.cmp(&b.route),
            }),
        }
    }
}

impl<'a> Branch<'a> {
    fn new(folder: Option<ContentPath>) -> Self {
        Self { folder, index: None, pages: Vec::new(), branches: Vec::new() }
    }

    /// The folder's name, or the empty text for the top.
    pub fn name(&self) -> &str {
        self.folder.as_ref().map_or("", ContentPath::file_name)
    }

    /// What the folder is called: its own page's title, else its name.
    pub fn title(&self) -> &str {
        self.index.map_or_else(|| self.name(), |page| page.title.as_str())
    }

    /// Whether `page` is in this folder or in a folder below it.
    pub fn holds(&self, page: &Page) -> bool {
        self.index.is_some_and(|index| index.id == page.id)
            || self.pages.iter().any(|held| held.id == page.id)
            || self.branches.iter().any(|branch| branch.holds(page))
    }

    /// Every page of the folder and below, in reading order: the folder's own page, its pages,
    /// then each folder in turn.
    pub fn walk(&self) -> Vec<&'a Page> {
        let mut found: Vec<&'a Page> = self.index.into_iter().chain(self.pages.iter().copied()).collect();
        for branch in &self.branches {
            found.extend(branch.walk());
        }
        found
    }

    /// Whether a page file stands for its folder: `index` or `README`, in any case.
    fn is_index(page: &Page) -> bool {
        let stem = page.file.stem();
        stem.eq_ignore_ascii_case("index") || stem.eq_ignore_ascii_case("readme")
    }

    /// Puts `page` in the folder `down` names below this one, one name per step. `root` is the
    /// folder the site starts in, which the top is.
    fn place(&mut self, page: &'a Page, down: &[&str], root: Option<&ContentPath>) {
        let Some((name, rest)) = down.split_first() else {
            if self.folder.is_some() && self.index.is_none() && Self::is_index(page) {
                self.index = Some(page);
            } else {
                self.pages.push(page);
            }
            return;
        };
        let joined = match self.folder.as_ref().or(root) {
            Some(parent) => format!("{parent}/{name}"),
            None => (*name).to_owned(),
        };
        // a folder named from the segments of a path is a path, so the page is not lost
        let Ok(path) = ContentPath::parse(joined) else {
            self.pages.push(page);
            return;
        };
        let at = self.branches.iter().position(|branch| branch.folder.as_ref() == Some(&path)).unwrap_or_else(|| {
            self.branches.push(Self::new(Some(path)));
            self.branches.len() - 1
        });
        if let Some(branch) = self.branches.get_mut(at) {
            branch.place(page, rest, root);
        }
    }

    /// Orders what was placed: pages by the folder's [`Order`] and the folders by weight. The
    /// top lists the navigation's pages first, in the navigation's order.
    fn arrange(&mut self, site: &Site) {
        let section = |folder: Option<&ContentPath>| {
            folder.and_then(|folder| site.sections.iter().find(|section| &section.folder == folder))
        };
        match self.folder.as_ref() {
            None => self.pages.sort_by_key(|page| site.nav.iter().position(|id| *id == page.id).unwrap_or(usize::MAX)),
            Some(folder) => section(Some(folder)).map_or(Order::Route, |section| section.order).sort(&mut self.pages),
        }
        self.branches.sort_by(|a, b| {
            let weight = |branch: &Self| section(branch.folder.as_ref()).map_or(0, |section| section.weight);
            weight(a).cmp(&weight(b)).then_with(|| a.name().cmp(b.name()))
        });
        for branch in &mut self.branches {
            branch.arrange(site);
        }
    }
}

impl Site {
    /// What `ink.toml` says of each folder, in path order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The pages in reading order as a tree of folders. The home, the not-found page and the
    /// pages outside the site's root are not in it.
    pub fn contents(&self) -> Branch<'_> {
        let root = match &self.root {
            Root::Folder(folder) => Some(folder),
            _ => None,
        };
        let mut top = Branch::new(None);
        for page in self.pages.iter().filter(|page| page.kind != PageKind::Home && page.route.as_str() != "/404") {
            let relative = match root {
                Some(root) => page.file.strip(root),
                None => Some(page.file.clone()),
            };
            let Some(relative) = relative else { continue };
            let names: Vec<&str> = relative.segments().collect();
            if let Some((_, down)) = names.split_last() {
                top.place(page, down, root);
            }
        }
        top.arrange(self);
        top
    }

    /// The pages before and after `page` in reading order; neither when the page is not in the
    /// contents.
    pub fn neighbours(&self, page: &Page) -> Neighbours<'_> {
        let order = self.contents().walk();
        let at = order.iter().position(|held| held.id == page.id);
        let step = |at: Option<usize>| at.and_then(|at| order.get(at).copied());
        Neighbours { previous: step(at.and_then(|at| at.checked_sub(1))), next: step(at.map(|at| at + 1)) }
    }
}
