//! The `[folder."path"]` tables: what a folder says to the pages in it.
//!
//! ```toml
//! [folder."decisions"]
//! kind = "page"        # every page in it is a page, though each has a `date`
//! weight = 10          # where the folder stands among its siblings: lower first
//! order = "route"      # how its pages are listed: route, title, newest or oldest
//! ```
//!
//! `kind` is a rung of the `page.kind` ladder (below the page's own `type`, above a posts folder
//! and a date), so the decision says it came from this table. `weight` and `order` are not
//! decisions: they are the [`Section`]s of the site that its navigation and its listings read.

use model::{ContentPath, PageKind, Section, Span, Spanned};

/// What one `[folder]` table says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    /// The folder, as a path in the snapshot.
    pub path: ContentPath,
    /// `kind = "..."`: what the pages in it are, with the whole `key = value`.
    pub kind: Option<Spanned<PageKind>>,
    /// `weight = N`: where it stands among its siblings.
    pub weight: Option<Spanned<i32>>,
    /// `order = "..."`: how its pages are listed.
    pub order: Option<Spanned<model::Order>>,
    /// The table's key, `"path"`.
    pub span: Span,
}

impl Folder {
    /// The folder with nothing said yet.
    pub(crate) const fn new(path: ContentPath, span: Span) -> Self {
        Self { path, kind: None, weight: None, order: None, span }
    }

    /// Its [`Section`], when it says a weight or an order.
    pub(crate) fn section(&self) -> Option<Section> {
        if self.weight.is_none() && self.order.is_none() {
            return None;
        }
        let weight = self.weight.as_ref().map_or(0, |weight| weight.value);
        let order = self.order.as_ref().map_or_else(model::Order::default, |order| order.value);
        Some(Section::new(self.path.clone(), weight, order))
    }
}
