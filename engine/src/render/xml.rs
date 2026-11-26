//! The XML the engine serves (Atom, RSS, the sitemap), written through `quick-xml`'s writer.
//! The one module that knows `quick-xml`: the rest of the engine hands it typed values.

use std::io;

use jiff::Timestamp;
use jiff::civil::Date;
use model::{Author, Tag};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText, Event};

const ATOM: &str = "http://www.w3.org/2005/Atom";
const SITEMAP: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";

/// One post in a feed.
#[derive(Debug)]
pub(crate) struct Item<'a> {
    pub(crate) title: &'a str,
    pub(crate) summary: Option<&'a str>,
    pub(crate) url: String,
    pub(crate) published: Timestamp,
    pub(crate) updated: Timestamp,
    /// `published` as RSS writes it (RFC 9110, `Fri, 01 Mar 2024 00:00:00 GMT`).
    pub(crate) published_rss: String,
    pub(crate) tags: &'a [Tag],
}

/// A feed: the site and its dated posts, newest first.
#[derive(Debug)]
pub(crate) struct Channel<'a> {
    pub(crate) title: &'a str,
    pub(crate) description: Option<&'a str>,
    /// Atom needs an author: with none decided, the site's title stands for them.
    pub(crate) authors: &'a [Author],
    pub(crate) language: &'a str,
    /// The site's home address.
    pub(crate) home: String,
    /// The newest change of any item.
    pub(crate) updated: Timestamp,
    pub(crate) items: Vec<Item<'a>>,
}

/// One page in the sitemap.
#[derive(Debug)]
pub(crate) struct Location {
    pub(crate) url: String,
    pub(crate) changed: Option<Date>,
}

fn element(out: &mut Writer<Vec<u8>>, name: &str, text: &str) -> io::Result<()> {
    out.create_element(name).write_text_content(BytesText::new(text)).map(drop)
}

fn declared() -> io::Result<Writer<Vec<u8>>> {
    let mut out = Writer::new(Vec::new());
    out.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
    Ok(out)
}

/// The Atom feed (RFC 4287), served at `own`.
pub(crate) fn atom(channel: &Channel<'_>, own: &str) -> io::Result<Vec<u8>> {
    let mut out = declared()?;
    out.create_element("feed").with_attributes([("xmlns", ATOM), ("xml:lang", channel.language)]).write_inner_content(
        |out| {
            element(out, "title", channel.title)?;
            element(out, "id", &channel.home)?;
            out.create_element("link")
                .with_attributes([("rel", "alternate"), ("href", &channel.home)])
                .write_empty()?;
            out.create_element("link").with_attributes([("rel", "self"), ("href", own)]).write_empty()?;
            channel.description.iter().try_for_each(|line| element(out, "subtitle", line))?;
            element(out, "updated", &channel.updated.to_string())?;
            let names: Vec<&str> = channel.authors.iter().map(AsRef::as_ref).collect();
            let names = if names.is_empty() { vec![channel.title] } else { names };
            for name in names {
                out.create_element("author").write_inner_content(|out| element(out, "name", name))?;
            }
            channel.items.iter().try_for_each(|item| atom_entry(out, item))
        },
    )?;
    Ok(out.into_inner())
}

fn atom_entry(out: &mut Writer<Vec<u8>>, item: &Item<'_>) -> io::Result<()> {
    out.create_element("entry").write_inner_content(|out| {
        element(out, "title", item.title)?;
        element(out, "id", &item.url)?;
        out.create_element("link").with_attributes([("rel", "alternate"), ("href", &item.url)]).write_empty()?;
        element(out, "published", &item.published.to_string())?;
        element(out, "updated", &item.updated.to_string())?;
        item.summary.iter().try_for_each(|summary| element(out, "summary", summary))?;
        for tag in item.tags {
            let term = [("term", tag.slug.as_str()), ("label", tag.written.as_str())];
            out.create_element("category").with_attributes(term).write_empty()?;
        }
        Ok(())
    })?;
    Ok(())
}

/// The RSS 2.0 feed, served at `own`.
pub(crate) fn rss(channel: &Channel<'_>, own: &str) -> io::Result<Vec<u8>> {
    let mut out = declared()?;
    let attributes = [("version", "2.0"), ("xmlns:atom", ATOM)];
    out.create_element("rss").with_attributes(attributes).write_inner_content(|out| {
        out.create_element("channel").write_inner_content(|out| {
            element(out, "title", channel.title)?;
            element(out, "link", &channel.home)?;
            element(out, "description", channel.description.unwrap_or(channel.title))?;
            element(out, "language", channel.language)?;
            let own = [("href", own), ("rel", "self"), ("type", "application/rss+xml")];
            out.create_element("atom:link").with_attributes(own).write_empty()?;
            channel.items.iter().try_for_each(|item| rss_item(out, item))
        })?;
        Ok(())
    })?;
    Ok(out.into_inner())
}

fn rss_item(out: &mut Writer<Vec<u8>>, item: &Item<'_>) -> io::Result<()> {
    out.create_element("item").write_inner_content(|out| {
        element(out, "title", item.title)?;
        element(out, "link", &item.url)?;
        out.create_element("guid")
            .with_attribute(("isPermaLink", "true"))
            .write_text_content(BytesText::new(&item.url))?;
        element(out, "pubDate", &item.published_rss)?;
        item.summary.iter().try_for_each(|summary| element(out, "description", summary))?;
        item.tags.iter().try_for_each(|tag| element(out, "category", &tag.written))
    })?;
    Ok(())
}

/// The sitemap (sitemaps.org 0.9).
pub(crate) fn sitemap(locations: &[Location]) -> io::Result<Vec<u8>> {
    let mut out = declared()?;
    out.create_element("urlset").with_attribute(("xmlns", SITEMAP)).write_inner_content(|out| {
        for location in locations {
            out.create_element("url").write_inner_content(|out| {
                element(out, "loc", &location.url)?;
                location.changed.iter().try_for_each(|day| element(out, "lastmod", &day.to_string()))
            })?;
        }
        Ok(())
    })?;
    Ok(out.into_inner())
}
