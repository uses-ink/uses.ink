//! The sitemap (sitemaps.org 0.9) of every page but the not-found page, a generated home among
//! them, and `robots.txt` pointing at it.

use std::fmt;

use model::{MediaType, Route, Site, Status};

use crate::error::Error;
use crate::render::output::{Body, Entry, Output, Owner};
use crate::render::{Artifact, Reserved};

/// `/sitemap.xml`.
#[derive(Debug)]
pub(crate) struct Sitemap;

impl Sitemap {
    const NAMESPACE: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";
}

impl Artifact for Sitemap {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        let not_found = Reserved::NotFound.route()?;
        let generated = site.home().is_none().then(|| (out.base.at(&Route::root()), None));
        let pages = site.pages().iter().filter(|page| page.route != not_found).map(|page| {
            (out.base.at(&page.route), page.updated.or(page.date).map(model::When::day).map(|day| day.to_string()))
        });
        let locations: Vec<(String, Option<String>)> = generated.into_iter().chain(pages).collect();
        out.xml(Reserved::Sitemap, MediaType::Xml, |xml, _own| {
            xml.parent("urlset", [("xmlns", Self::NAMESPACE)], |xml| {
                locations.iter().try_for_each(|(url, changed)| {
                    xml.parent("url", [], |xml| {
                        xml.leaf("loc", url)?;
                        changed.iter().try_for_each(|day| xml.leaf("lastmod", day))
                    })
                })
            })
        })
    }
}

/// `/robots.txt`: everything may be crawled; the sitemap says what there is.
#[derive(Debug)]
pub(crate) struct Robots;

impl Artifact for Robots {
    fn files(&self, _site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        let sitemap = out.base.at(&Reserved::Sitemap.route()?);
        let route = Reserved::Robots.route()?;
        let body = Body::Bytes(Allowed { sitemap: &sitemap }.to_string().into_bytes());
        out.serve(Entry { route, media: MediaType::Text, status: Status::Ok, body, owner: Owner::Engine });
        Ok(())
    }
}

/// The words of `robots.txt`.
#[derive(Debug)]
struct Allowed<'a> {
    sitemap: &'a str,
}

impl fmt::Display for Allowed<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(out, "User-agent: *")?;
        writeln!(out, "Allow: /")?;
        writeln!(out)?;
        writeln!(out, "Sitemap: {}", self.sitemap)
    }
}
