//! The page around a document: the head, the site's header and navigation, the title with its
//! date and tags, the home's listing, and the footer.

use std::fmt;

use engine::{BaseUrl, Record};
use model::highlight::Highlight;
use model::{Block, Page, PageKind, Site, When};
use unspliced::{HERE, Halt, Lazy, Render, Sink, Split, Url, html};

use crate::document::{Source, Unwritten, Writer};
use crate::style::quiet;

/// The routes the theme links from every page's head.
pub(crate) const STYLE: &str = "/style.css";
pub(crate) const CODE: &str = "/code.css";
const ATOM: &str = "/feed.xml";
const RSS: &str = "/rss.xml";

/// One whole page.
#[derive(Debug)]
pub(crate) struct Layout<'a, H> {
    pub(crate) source: Source<'a, H>,
    pub(crate) base: &'a BaseUrl,
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

    /// Whether the engine writes feeds: when a post is dated.
    fn has_feeds(&self) -> bool {
        self.source.site.pages().iter().any(|page| page.kind == PageKind::Post && page.date.is_some())
    }

    /// Writes the page into `out`, its document where the shell holds [`HERE`], recording what
    /// the document's writer finds worth saying into `record`.
    pub(crate) fn write<R: Record + ?Sized>(&self, out: &mut dyn Sink, record: &mut R) -> Result<(), Unwritten> {
        let (page, identity) = (self.source.page, self.source.site.identity());
        let shell = html! {
            doctype
            html lang=(identity.language.as_ref()) {
                (Lazy(|out: &mut dyn Sink| self.head(out)))
                body {
                    div.quiet::PAGE {
                        header.quiet::HEAD {
                            a.quiet::SITE href=(Url::new("/")) { (&identity.title) }
                            (Lazy(|out: &mut dyn Sink| self.nav(out)))
                        }
                        main {
                            article {
                                @if self.is_titled() {
                                    h1 { (&page.title) }
                                }
                                (Lazy(|out: &mut dyn Sink| self.meta(out)))
                                (HERE)
                            }
                            @if page.kind == PageKind::Home { (Lazy(|out: &mut dyn Sink| self.listing(out))) }
                        }
                        footer.quiet::FOOT { p { (&identity.title) } }
                    }
                }
            }
        };
        let split = Split::new(&shell).map_err(Unwritten::Marks)?;
        split.head(out).map_err(Unwritten::Halt)?;
        Writer::new(self.source, out, record).write()?;
        split.tail(out).map_err(Unwritten::Halt)
    }

    fn head(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let (page, identity) = (self.source.page, self.source.site.identity());
        let title = Title(page, self.source.site).to_string();
        let description = page.summary.as_deref().or(identity.description.as_deref());
        let canonical = self.base.at(&page.route);
        let kind = if page.kind == PageKind::Post { "article" } else { "website" };
        let feeds = self.has_feeds();
        html! {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";
            title { (&title) }
            @if let Some(description) = description { meta name="description" content=(description); }
            @if let Ok(url) = Url::checked(&canonical) { link rel="canonical" href=(url); }
            link rel="stylesheet" href=(Url::new(STYLE));
            link rel="stylesheet" href=(Url::new(CODE));
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
        let others: Vec<&Page> = if self.source.site.home().is_none() {
            self.source
                .site
                .pages()
                .iter()
                .filter(|page| page.kind == PageKind::Page && page.route.as_str() != "/404")
                .collect()
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
