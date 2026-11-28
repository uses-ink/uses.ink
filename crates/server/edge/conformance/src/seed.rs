//! What a case asks a harness to serve.

use model::{Hash, Manifest, MediaType, Route, Served};

/// One page of a seeded site: a route and the bytes behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// The route, as the manifest writes it.
    pub route: String,
    /// What it serves.
    pub bytes: Vec<u8>,
}

/// A site to serve: the host it answers at, and its pages. The bytes of two pages differ, so
/// every page has a blob of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    /// The host name.
    pub host: String,
    /// The pages, in route order.
    pub pages: Vec<Page>,
}

impl Seed {
    /// A site at `host` with no pages yet.
    pub fn at(host: &str) -> Self {
        Self { host: host.to_owned(), pages: Vec::new() }
    }

    /// The same site with one page more.
    #[must_use]
    pub fn page(mut self, route: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.pages.push(Page { route: route.to_owned(), bytes: bytes.into() });
        self
    }

    /// The manifest of the pages: every route is a page of HTML with no headers of its own.
    pub fn manifest(&self) -> Manifest {
        let served: Vec<Served> = self
            .pages
            .iter()
            .map(|page| {
                Served::new(Route::parse(&page.route).unwrap(), Hash::of(&page.bytes), MediaType::Html, Vec::new())
            })
            .collect();
        Manifest::new(served).unwrap()
    }
}
