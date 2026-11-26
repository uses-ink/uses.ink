//! The feeds of a site's dated posts, newest first, in each syndication format of `S`. An undated
//! post is left out and said so; a site with no dated post has no feeds, and says so.

use std::io;

use jiff::Timestamp;
use jiff::civil::Time;
use jiff::fmt::rfc2822::DateTimePrinter;
use jiff::tz::Offset;
use model::{Announcement, Author, MediaType, Notice, Page, PageKind, Route, Site, Subject, Tag, When};

use crate::error::Error;
use crate::render::output::Output;
use crate::render::xml::Xml;
use crate::render::{Artifact, Reserved};

/// The Atom namespace, which RSS also uses for its self link.
const ATOM: &str = "http://www.w3.org/2005/Atom";

/// A syndication format a feed is written in. A tuple of formats writes each, in order.
pub(crate) trait Syndication {
    /// Serves `channel` in this format.
    fn serve(&self, channel: &Channel<'_>, out: &mut Output<'_>) -> Result<(), Error>;
}

/// The site's feeds, one per format of `S`, from one channel: its posts are gathered, and their
/// absence said, once.
#[derive(Debug, Default)]
pub(crate) struct Feed<S> {
    formats: S,
}

impl<S: Syndication> Artifact for Feed<S> {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        match Channel::of(site, out)? {
            Some(channel) => self.formats.serve(&channel, out),
            None => Ok(()),
        }
    }
}

/// Implements [`Syndication`] for a tuple of formats, each served in order. The same impl over
/// tuples of every length up to three, which a generic cannot write.
///
/// ```text
/// syndications!(A 0, B 1);
/// // makes `impl<A: Syndication, B: Syndication> Syndication for (A, B)`
/// ```
macro_rules! syndications {
    ($($format:ident $at:tt),+) => {
        impl<$($format: Syndication),+> Syndication for ($($format,)+) {
            fn serve(&self, channel: &Channel<'_>, out: &mut Output<'_>) -> Result<(), Error> {
                $(self.$at.serve(channel, out)?;)+
                Ok(())
            }
        }
    };
}

syndications!(A 0);
syndications!(A 0, B 1);
syndications!(A 0, B 1, C 2);

/// One post in a feed.
#[derive(Debug)]
pub(crate) struct Item<'a> {
    title: &'a str,
    summary: Option<&'a str>,
    url: String,
    published: Timestamp,
    updated: Timestamp,
    /// `published` as RSS writes it (RFC 9110, `Fri, 01 Mar 2024 00:00:00 GMT`).
    published_rss: String,
    tags: &'a [Tag],
}

impl<'a> Item<'a> {
    /// The item of `page`, dated `date`, linked under the render's base.
    fn of(page: &'a Page, date: When, out: &Output<'_>) -> Result<Self, Error> {
        let published = Self::instant(date)?;
        let updated = page.updated.map(Self::instant).transpose()?.map_or(published, |updated| updated.max(published));
        let published_rss = DateTimePrinter::new()
            .timestamp_to_rfc9110_string(&published)
            .map_err(|source| Error::Date { when: date, source })?;
        let summary = page.summary.as_deref();
        let url = out.base.at(&page.route);
        Ok(Self { title: &page.title, summary, url, published, updated, published_rss, tags: &page.tags })
    }

    /// A feed needs an instant: a bare day is its first instant in UTC, as both formats require
    /// a time the author never wrote.
    fn instant(when: When) -> Result<Timestamp, Error> {
        if let When::Instant(at) = when {
            return Ok(at);
        }
        let midnight = when.day().to_datetime(Time::midnight());
        Offset::UTC.to_timestamp(midnight).map_err(|source| Error::Date { when, source })
    }
}

/// A feed: the site and its dated posts, newest first.
#[derive(Debug)]
pub(crate) struct Channel<'a> {
    title: &'a str,
    description: Option<&'a str>,
    /// Atom needs an author: with none decided, the site's title stands for them.
    authors: &'a [Author],
    language: &'a str,
    /// The site's home address.
    home: String,
    /// The newest change of any item.
    updated: Timestamp,
    items: Vec<Item<'a>>,
}

impl<'a> Channel<'a> {
    /// The channel of `site`'s dated posts, or `None` when no post is dated.
    fn of(site: &'a Site, out: &mut Output<'_>) -> Result<Option<Self>, Error> {
        let mut items = Vec::new();
        for page in site.pages().iter().filter(|page| page.kind == PageKind::Post) {
            match page.date {
                Some(date) => items.push(Item::of(page, date, out)?),
                None => out.notice(Notice::new(Announcement::FeedPostUndated).on(Subject::File(page.file.clone()))),
            }
        }
        items.sort_by(|a, b| b.published.cmp(&a.published).then_with(|| a.url.cmp(&b.url)));
        let Some(updated) = items.iter().map(|item| item.updated).max() else {
            out.notice(Notice::new(Announcement::FeedPostNone).on(Subject::Site));
            return Ok(None);
        };
        let identity = site.identity();
        Ok(Some(Self {
            title: &identity.title,
            description: identity.description.as_deref(),
            authors: &identity.authors,
            language: identity.language.as_ref(),
            home: out.base.at(&Route::root()),
            updated,
            items,
        }))
    }
}

/// Atom (RFC 4287), at `/feed.xml`.
#[derive(Debug, Default)]
pub(crate) struct Atom;

impl Syndication for Atom {
    fn serve(&self, channel: &Channel<'_>, out: &mut Output<'_>) -> Result<(), Error> {
        out.xml(Reserved::Atom, MediaType::Atom, |xml, own| Self::channel(channel, own, xml))
    }
}

impl Atom {
    fn channel(channel: &Channel<'_>, own: &str, xml: &mut Xml) -> io::Result<()> {
        xml.parent("feed", [("xmlns", ATOM), ("xml:lang", channel.language)], |xml| {
            xml.leaf("title", channel.title)?;
            xml.leaf("id", &channel.home)?;
            xml.empty("link", [("rel", "alternate"), ("href", &channel.home)])?;
            xml.empty("link", [("rel", "self"), ("href", own)])?;
            channel.description.iter().try_for_each(|line| xml.leaf("subtitle", line))?;
            xml.leaf("updated", &channel.updated.to_string())?;
            let names: Vec<&str> = channel.authors.iter().map(AsRef::as_ref).collect();
            let names = if names.is_empty() { vec![channel.title] } else { names };
            for name in names {
                xml.parent("author", [], |xml| xml.leaf("name", name))?;
            }
            channel.items.iter().try_for_each(|item| Self::entry(item, xml))
        })
    }

    fn entry(item: &Item<'_>, xml: &mut Xml) -> io::Result<()> {
        xml.parent("entry", [], |xml| {
            xml.leaf("title", item.title)?;
            xml.leaf("id", &item.url)?;
            xml.empty("link", [("rel", "alternate"), ("href", &item.url)])?;
            xml.leaf("published", &item.published.to_string())?;
            xml.leaf("updated", &item.updated.to_string())?;
            item.summary.iter().try_for_each(|summary| xml.leaf("summary", summary))?;
            item.tags.iter().try_for_each(|tag| {
                xml.empty("category", [("term", tag.slug.as_str()), ("label", tag.written.as_str())])
            })
        })
    }
}

/// RSS 2.0, at `/rss.xml`.
#[derive(Debug, Default)]
pub(crate) struct Rss;

impl Syndication for Rss {
    fn serve(&self, channel: &Channel<'_>, out: &mut Output<'_>) -> Result<(), Error> {
        out.xml(Reserved::Rss, MediaType::Rss, |xml, own| Self::channel(channel, own, xml))
    }
}

impl Rss {
    fn channel(channel: &Channel<'_>, own: &str, xml: &mut Xml) -> io::Result<()> {
        xml.parent("rss", [("version", "2.0"), ("xmlns:atom", ATOM)], |xml| {
            xml.parent("channel", [], |xml| {
                xml.leaf("title", channel.title)?;
                xml.leaf("link", &channel.home)?;
                xml.leaf("description", channel.description.unwrap_or(channel.title))?;
                xml.leaf("language", channel.language)?;
                xml.empty("atom:link", [("href", own), ("rel", "self"), ("type", "application/rss+xml")])?;
                channel.items.iter().try_for_each(|item| Self::item(item, xml))
            })
        })
    }

    fn item(item: &Item<'_>, xml: &mut Xml) -> io::Result<()> {
        xml.parent("item", [], |xml| {
            xml.leaf("title", item.title)?;
            xml.leaf("link", &item.url)?;
            xml.text("guid", [("isPermaLink", "true")], &item.url)?;
            xml.leaf("pubDate", &item.published_rss)?;
            item.summary.iter().try_for_each(|summary| xml.leaf("description", summary))?;
            item.tags.iter().try_for_each(|tag| xml.leaf("category", &tag.written))
        })
    }
}
