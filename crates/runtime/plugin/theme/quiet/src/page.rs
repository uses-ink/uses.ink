//! The page around a document: the head, the site's header and navigation, the title with its
//! date and tags, the home's listing, and the footer. With `navigation = "docs"` it also draws the
//! site's folders beside the page, the page's own outline and the links to the pages before and
//! after it, all from the site and the page, with no script.

use std::fmt;

use engine::BaseUrl;
use model::highlight::Highlight;
use model::{Block, Branch, Heading, Page, PageKind, Site, When};
use unspliced::keyword::AriaCurrent;
use unspliced::{Class, HERE, Halt, Id, Lazy, Render, Sink, Split, Url, html};

use crate::document::{Source, Unwritten, Writer};
use crate::notebook::{Notebook, hand};
use crate::style::quiet;
use crate::tokens::{Flow, Look, Navigation, Tokens};

/// The routes the theme links from every page's head.
pub(crate) const STYLE: &str = "/style.css";
pub(crate) const CODE: &str = "/code.css";
/// The stylesheet of a look other than the theme's own.
pub(crate) const LOOK: &str = "/look.css";
/// The mark the look draws before the site's name on a page of panels.
const LOGO: &str = "/marks/logo-open.svg";
/// The small mark of the signature.
const MARK: &str = "/marks/mark.svg";
const ATOM: &str = "/feed.xml";
const RSS: &str = "/rss.xml";

/// One whole page.
#[derive(Debug)]
pub(crate) struct Layout<'a, H> {
    pub(crate) source: Source<'a, H>,
    pub(crate) base: &'a BaseUrl,
    pub(crate) tokens: Tokens,
}

/// A page's `<title>`: the site alone on the home, else `<page> - <site>`.
#[derive(Clone, Copy, Debug)]
struct Title<'a>(&'a Page, &'a Site);

impl fmt::Display for Title<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let site = &self.1.identity().title;
        if self.0.kind == PageKind::Home || self.0.title == *site {
            out.write_str(site)
        } else {
            write!(out, "{} - {site}", self.0.title)
        }
    }
}

impl<H: Highlight> Layout<'_, H> {
    /// Whether the page needs the title as its heading: not when its text starts with one.
    fn is_titled(&self) -> bool {
        !matches!(self.source.page.body.blocks.first(), Some(Block::Heading { level, .. }) if level.get() == 1)
    }

    /// Whether the page is drawn with the site's folders, its outline and its neighbours.
    fn is_docs(&self) -> bool {
        self.tokens.navigation == Navigation::Docs
    }

    /// Whether the page wears the hand-drawn look.
    fn is_drawn(&self) -> bool {
        self.tokens.look == Look::UsesInk
    }

    /// Whether the page is a page of panels, which has no footer of the theme's: its last panel is one.
    fn is_panels(&self) -> bool {
        self.tokens.layout == Flow::Panels
    }

    /// Whether the engine writes feeds: when a post is dated.
    fn has_feeds(&self) -> bool {
        self.source.site.pages().iter().any(|page| page.kind == PageKind::Post && page.date.is_some())
    }

    /// Writes the page into `out`, its document where the shell holds [`HERE`], recording what
    /// the document's writer finds worth saying into `record`.
    pub(crate) fn write<R: engine::Sink + ?Sized>(&self, out: &mut dyn Sink, record: &mut R) -> Result<(), Unwritten> {
        let (page, identity) = (self.source.page, self.source.site.identity());
        let docs = self.is_docs();
        let panels = self.is_panels();
        let shell = html! {
            doctype
            html lang=(identity.language.as_ref()) {
                (Lazy(|out: &mut dyn Sink| self.head(out)))
                body class=(panels.then_some(hand::SHOW)) {
                    div.quiet::PAGE class=(docs.then_some(quiet::WIDE)) {
                        header.quiet::HEAD {
                            a.quiet::SITE href=(Url::new("/")) {
                                @if panels { img.hand::LOGO src=(Url::new(LOGO)) alt="" width=(44) height=(44); }
                                (&identity.title)
                            }
                            (Lazy(|out: &mut dyn Sink| self.nav(out)))
                        }
                        @if docs {
                            (Lazy(|out: &mut dyn Sink| self.side(out)))
                            (Lazy(|out: &mut dyn Sink| self.outline(out)))
                        }
                        main class=(docs.then_some(quiet::BODY)) {
                            article {
                                @if self.is_titled() {
                                    h1 { (&page.title) }
                                }
                                (Lazy(|out: &mut dyn Sink| self.meta(out)))
                                (HERE)
                            }
                            @if docs { (Lazy(|out: &mut dyn Sink| self.pager(out))) }
                            @if page.kind == PageKind::Home { (Lazy(|out: &mut dyn Sink| self.listing(out))) }
                        }
                        (Lazy(|out: &mut dyn Sink| self.foot(out)))
                    }
                }
            }
        };
        let split = Split::new(&shell).map_err(Unwritten::Marks)?;
        split.head(out).map_err(Unwritten::Halt)?;
        Writer::new(self.source, out, record).arranged(self.tokens.layout).write()?;
        split.tail(out).map_err(Unwritten::Halt)
    }

    /// The footer of the page: the site's name, or in the drawn look the signature; a page of
    /// panels has none, since its last panel is one.
    fn foot(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        if self.is_panels() {
            Ok(())
        } else if self.is_drawn() {
            html! { footer.quiet::FOOT { (Lazy(|out: &mut dyn Sink| self.signature(out))) } }.render(out)
        } else {
            html! { footer.quiet::FOOT { p { (&self.source.site.identity().title) } } }.render(out)
        }
    }

    /// The footer of the drawn look: the feed when there is one, and the one line uses.ink signs.
    fn signature(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let feeds = self.has_feeds();
        html! {
            @if feeds { a href=(Url::new(ATOM)) { "feed" } }
            @if let Ok(home) = Url::checked("https://uses.ink") {
                a.hand::MADE href=(home) { img src=(Url::new(MARK)) alt="" width=(22) height=(22); "made with uses.ink" }
            }
        }
        .render(out)
    }

    fn head(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let (page, identity) = (self.source.page, self.source.site.identity());
        let title = Title(page, self.source.site).to_string();
        let description = page.summary.as_deref().or(identity.description.as_deref());
        let canonical = self.base.at(&page.route);
        let kind = if page.kind == PageKind::Post { "article" } else { "website" };
        let feeds = self.has_feeds();
        let look = self.tokens.look != Look::Quiet;
        html! {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";
            title { (&title) }
            @if let Some(description) = description { meta name="description" content=(description); }
            @if let Ok(url) = Url::checked(&canonical) { link rel="canonical" href=(url); }
            link rel="stylesheet" href=(Url::new(STYLE));
            link rel="stylesheet" href=(Url::new(CODE));
            @if look { link rel="stylesheet" href=(Url::new(LOOK)); link rel="stylesheet" href=(Url::new(Notebook::SHEET)); }
            @if feeds {
                link rel="alternate" type="application/atom+xml" title=(&identity.title) href=(Url::new(ATOM));
                link rel="alternate" type="application/rss+xml" title=(&identity.title) href=(Url::new(RSS));
            }
            meta property="og:title" content=(&page.title);
            meta property="og:type" content=(kind);
            meta property="og:site_name" content=(&identity.title);
            meta property="og:url" content=(&canonical);
            @if let Some(description) = description { meta property="og:description" content=(description); }
        }
        }
        .render(out)
    }

    fn nav(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let pages: Vec<&Page> = self.source.site.nav().iter().filter_map(|id| self.source.site.page(*id)).collect();
        if pages.is_empty() {
            return Ok(());
        }
        html! {
            nav aria-label="Site" { ul { @for page in &pages { li { (Lazy(|out: &mut dyn Sink| Self::to(page, out))) } } } }
        }
        .render(out)
    }

    /// The site's folders: the top's pages, then each folder as a group that is open when it
    /// holds the page. The page's own entry is the current one.
    fn side(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let contents = self.source.site.contents();
        html! { nav.quiet::SIDE aria-label="Contents" { (Lazy(|out: &mut dyn Sink| self.branch(&contents, out))) } }
            .render(out)
    }

    /// One folder's list: its own page first, then its pages, then its folders.
    fn branch(&self, branch: &Branch<'_>, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            ul {
                @if let Some(index) = branch.index { li { (Lazy(|out: &mut dyn Sink| self.entry(index, out))) } }
                @for page in &branch.pages { li { (Lazy(|out: &mut dyn Sink| self.entry(page, out))) } }
                @for folder in &branch.branches { li { (Lazy(|out: &mut dyn Sink| self.group(folder, out))) } }
            }
        }
        .render(out)
    }

    /// A folder as a group of links that a reader can fold, with no script: a `details`.
    fn group(&self, folder: &Branch<'_>, out: &mut dyn Sink) -> Result<(), Halt> {
        let open = folder.holds(self.source.page);
        html! {
            details open=(open) {
                summary { (folder.title()) }
                (Lazy(|out: &mut dyn Sink| self.branch(folder, out)))
            }
        }
        .render(out)
    }

    /// A link to `page`, marked as the current page when it is the one being written.
    fn entry(&self, page: &Page, out: &mut dyn Sink) -> Result<(), Halt> {
        let route = page.route.to_string();
        let here = page.id == self.source.page.id;
        match Url::checked(&route) {
            Ok(url) => html! {
                a class=(here.then_some(quiet::CURRENT)) href=(url) aria-current=(here.then_some(AriaCurrent::Page)) {
                    (&page.title)
                }
            }
            .render(out),
            Err(_) => page.title.render(out),
        }
    }

    /// The headings below the title, each a link to itself, when there are two or more.
    fn outline(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let found: Vec<Heading> =
            self.source.page.body.outline().into_iter().filter(|heading| heading.level.get() >= 2).collect();
        if found.len() < 2 {
            return Ok(());
        }
        html! {
            nav.quiet::OUTLINE aria-label="On this page" {
                p.quiet::RUBRIC { "On this page" }
                ul {
                    @for heading in &found {
                        li class=(Self::depth(heading)) { (Lazy(|out: &mut dyn Sink| Self::jump(heading, out))) }
                    }
                }
            }
        }
        .render(out)
    }

    /// The indent of a heading's entry: none for a section, one step for a subsection, two below.
    fn depth(heading: &Heading) -> Option<Class> {
        match heading.level.get() {
            2 => None,
            3 => Some(quiet::DEEP),
            _ => Some(quiet::DEEPER),
        }
    }

    /// A link to a heading of this page.
    fn jump(heading: &Heading, out: &mut dyn Sink) -> Result<(), Halt> {
        match Id::checked(heading.anchor.as_str()) {
            Ok(id) => html! { a href=(Url::to(id)) { (&heading.text) } }.render(out),
            Err(_) => heading.text.render(out),
        }
    }

    /// The links to the page before and the page after, in the order the folders list them.
    fn pager(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let near = self.source.site.neighbours(self.source.page);
        if near.previous.is_none() && near.next.is_none() {
            return Ok(());
        }
        html! {
            nav.quiet::PAGER aria-label="Previous and next" {
                @if let Some(page) = near.previous { (Lazy(|out: &mut dyn Sink| Self::step(page, "Previous", quiet::BEFORE, out))) }
                @if let Some(page) = near.next { (Lazy(|out: &mut dyn Sink| Self::step(page, "Next", quiet::AFTER, out))) }
            }
        }
        .render(out)
    }

    /// One link of the pager: what it is, then the page's title.
    fn step(page: &Page, hint: &str, side: Class, out: &mut dyn Sink) -> Result<(), Halt> {
        match Url::checked(&page.route.to_string()) {
            Ok(url) => html! { a class=(side) href=(url) { span.quiet::HINT { (hint) } (&page.title) } }.render(out),
            Err(_) => page.title.render(out),
        }
    }

    /// A link to `page` with its title.
    fn to(page: &Page, out: &mut dyn Sink) -> Result<(), Halt> {
        let route = page.route.to_string();
        match Url::checked(&route) {
            Ok(url) => html! { a href=(url) { (&page.title) } }.render(out),
            Err(_) => page.title.render(out),
        }
    }

    /// The date and tags under the title.
    fn meta(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let page = self.source.page;
        if page.date.is_none() && page.tags.is_empty() {
            return Ok(());
        }
        html! {
            div.quiet::META {
                @if let Some(date) = page.date { (Lazy(move |out: &mut dyn Sink| Self::date(date, out))) }
                @if !page.tags.is_empty() {
                    ul.quiet::TAGS aria-label="Tags" { @for tag in &page.tags { li { "#" (&tag.written) } } }
                }
            }
        }
        .render(out)
    }

    fn date(date: When, out: &mut dyn Sink) -> Result<(), Halt> {
        let day = date.day().to_string();
        html! { time datetime=(&day) { (&day) } }.render(out)
    }

    /// On the home: the posts, newest first; on a generated home, every other page after them.
    fn listing(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let mut posts: Vec<&Page> =
            self.source.site.pages().iter().filter(|page| page.kind == PageKind::Post).collect();
        posts.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.route.cmp(&b.route)));
        // in the order the site's folders list them: the navigation's pages first, then each folder
        let others: Vec<&Page> = if self.source.site.home().is_none() {
            self.source.site.contents().walk().into_iter().filter(|page| page.kind == PageKind::Page).collect()
        } else {
            Vec::new()
        };
        let list = |pages: &[&Page], out: &mut dyn Sink| {
            html! {
                ul.quiet::LISTING {
                    @for page in pages {
                        li {
                            @if let Some(date) = page.date { (Lazy(move |out: &mut dyn Sink| Self::date(date, out))) " " }
                            (Lazy(|out: &mut dyn Sink| Self::to(page, out)))
                        }
                    }
                }
            }
            .render(out)
        };
        if !posts.is_empty() {
            html! { h2 { "Posts" } }.render(out)?;
            list(&posts, out)?;
        }
        if !others.is_empty() {
            html! { h2 { "Pages" } }.render(out)?;
            list(&others, out)?;
        }
        Ok(())
    }
}
