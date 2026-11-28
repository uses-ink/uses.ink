//! The cases.

use std::collections::BTreeMap;

use edge::{
    Authority, Blob as _, BlobError, Blobs as _, Body, Chunks as _, Hostname, Name, Request, Scheme, Sites as _, Span,
    Status,
};
use model::{Hash, Route};

use crate::{Harness, Seed};

/// What a case found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// The adapter did what the case asks.
    Held,
    /// The adapter does not offer what the case tests, for this reason. It is not a pass.
    NotOffered(&'static str),
}

/// One case of the suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Case {
    /// A host resolves to its deploy, and an unknown host to none.
    Resolves,
    /// A blob opens with its length, and a hash the deploy does not hold is missing.
    Opens,
    /// A whole blob and a span of it read back as written.
    Reads,
    /// A blob is read in the scope of its own site only.
    Scoped,
    /// A page is served through the edge with its type and length, and a route the deploy lacks is a `404`.
    Serves,
    /// A `Range` is a `206` with the span, through the edge.
    Ranges,
    /// A deploy of more than a hundred files is read whole.
    Many,
    /// A suspended site answers `403` and no bytes.
    Suspends,
    /// A custom domain serves the site it was proved for.
    Aliases,
}

impl Case {
    /// Every case, in the order the suite runs them.
    pub const ALL: &'static [Self] = &[
        Self::Resolves,
        Self::Opens,
        Self::Reads,
        Self::Scoped,
        Self::Serves,
        Self::Ranges,
        Self::Many,
        Self::Suspends,
        Self::Aliases,
    ];

    /// Its name: the name of its native test.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Resolves => "resolves_a_host_to_its_deploy",
            Self::Opens => "opens_a_blob_with_its_length",
            Self::Reads => "reads_a_whole_blob_and_a_span",
            Self::Scoped => "reads_a_blob_in_its_own_sites_scope_only",
            Self::Serves => "serves_a_page_and_refuses_a_route_it_lacks",
            Self::Ranges => "answers_a_range_with_the_span",
            Self::Many => "reads_a_deploy_of_more_than_a_hundred_files",
            Self::Suspends => "answers_403_and_no_bytes_for_a_suspended_site",
            Self::Aliases => "serves_a_custom_domain_with_its_sites_deploy",
        }
    }

    /// The case called `name`.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|case| case.name() == name)
    }

    /// Runs the case on a harness whose stores are empty.
    pub async fn run<H: Harness>(self, harness: &H) -> Outcome {
        match self {
            Self::Resolves => Self::resolves(harness).await,
            Self::Opens => Self::opens(harness).await,
            Self::Reads => Self::reads(harness).await,
            Self::Scoped => Self::scoped(harness).await,
            Self::Serves => Self::serves(harness).await,
            Self::Ranges => Self::ranges(harness).await,
            Self::Many => Self::many(harness).await,
            Self::Suspends => Self::suspends(harness).await,
            Self::Aliases => Self::aliases(harness).await,
        }
    }

    fn host(name: &str) -> Hostname {
        Authority::try_from(name.as_bytes()).unwrap().hostname
    }

    fn tea() -> Seed {
        Seed::at("tea.example.test").page("/tea", "<h1>a pot of tea</h1>").page("/coffee", "<h1>a pot of coffee</h1>")
    }

    /// The bytes of `span` of an opened blob.
    async fn drain<B: edge::Blob>(blob: B, span: Span) -> Vec<u8> {
        let mut chunks = blob.read(span);
        let mut bytes = Vec::new();
        while let Some(chunk) = chunks.next().await.unwrap() {
            bytes.extend_from_slice(&chunk);
        }
        bytes
    }

    /// A `GET` of `route` at `host`, with these header lines more.
    async fn get<H: Harness>(harness: &H, host: &str, route: &str, more: &[(&str, &str)]) -> Got {
        let edge = harness.edge().unwrap();
        let mut asked = Request::new("GET", route, Scheme::Plain);
        asked.extend([(&b"host"[..], host.as_bytes())]);
        asked.extend(more.iter().map(|(name, value)| (name.as_bytes(), value.as_bytes())));
        let answer = edge.respond(&asked).await;
        let response = answer.response;
        let length = response.field(Name::ContentLength).map(|value| value.as_str().to_owned());
        let range = response.field(Name::ContentRange).map(|value| value.as_str().to_owned());
        let body = match response.body {
            Body::Blob { blob, span } => Self::drain(blob, span).await,
            Body::Text(text) => text.into_bytes(),
            _ => Vec::new(),
        };
        Got { status: response.status, length, range, body }
    }

    async fn resolves<H: Harness>(harness: &H) -> Outcome {
        let seed = Self::tea();
        let manifest = harness.publish(&seed).await.unwrap();
        let found = harness.sites().resolve(&Self::host("tea.example.test")).await.unwrap().unwrap();
        assert_eq!(found.deploy.manifest.id(), manifest.id());
        let routes: Vec<String> =
            found.deploy.manifest.entries().iter().map(|entry| entry.route().to_string()).collect();
        let mut routes = routes;
        routes.sort();
        assert_eq!(routes, ["/coffee", "/tea"]);
        assert!(harness.sites().resolve(&Self::host("nobody.example.test")).await.unwrap().is_none());
        assert!(harness.sites().suspended(&Self::host("tea.example.test")).await.unwrap().is_none());
        Outcome::Held
    }

    async fn opens<H: Harness>(harness: &H) -> Outcome {
        let seed = Self::tea();
        harness.publish(&seed).await.unwrap();
        let found = harness.sites().resolve(&Self::host("tea.example.test")).await.unwrap().unwrap();
        for page in &seed.pages {
            let blob = harness.blobs().open(&found.scope, Hash::of(&page.bytes)).await.unwrap();
            assert_eq!(blob.length(), page.bytes.len() as u64);
        }
        let absent = Hash::of("no site holds this");
        let refused = harness.blobs().open(&found.scope, absent).await.err().unwrap();
        assert!(matches!(refused, BlobError::Missing { hash } if hash == absent), "{refused:?}");
        Outcome::Held
    }

    async fn reads<H: Harness>(harness: &H) -> Outcome {
        let seed = Self::tea();
        harness.publish(&seed).await.unwrap();
        let found = harness.sites().resolve(&Self::host("tea.example.test")).await.unwrap().unwrap();
        let bytes = &seed.pages[0].bytes;
        let hash = Hash::of(bytes);
        let blob = harness.blobs().open(&found.scope, hash).await.unwrap();
        assert_eq!(Self::drain(blob, Span::whole(bytes.len() as u64)).await, *bytes);
        let blob = harness.blobs().open(&found.scope, hash).await.unwrap();
        assert_eq!(Self::drain(blob, Span { start: 4, length: 9 }).await, bytes[4..13]);
        let blob = harness.blobs().open(&found.scope, hash).await.unwrap();
        let tail = Span { start: bytes.len() as u64 - 1, length: 1 };
        assert_eq!(Self::drain(blob, tail).await, bytes[bytes.len() - 1..]);
        Outcome::Held
    }

    async fn scoped<H: Harness>(harness: &H) -> Outcome {
        harness.publish(&Self::tea()).await.unwrap();
        let other = Seed::at("other.example.test").page("/other", "<h1>another site</h1>");
        harness.publish(&other).await.unwrap();
        let found = harness.sites().resolve(&Self::host("tea.example.test")).await.unwrap().unwrap();
        let hash = Hash::of(&other.pages[0].bytes);
        let refused = harness.blobs().open(&found.scope, hash).await.err().unwrap();
        assert!(matches!(refused, BlobError::Missing { .. }), "{refused:?}");
        Outcome::Held
    }

    async fn serves<H: Harness>(harness: &H) -> Outcome {
        let seed = Self::tea();
        harness.publish(&seed).await.unwrap();
        let page = Self::get(harness, "tea.example.test", "/tea", &[]).await;
        assert_eq!(page.status, Status::Ok);
        assert_eq!(page.body, seed.pages[0].bytes);
        assert_eq!(page.length, Some(seed.pages[0].bytes.len().to_string()));
        let lacking = Self::get(harness, "tea.example.test", "/water", &[]).await;
        assert_eq!(lacking.status, Status::NotFound);
        let unknown = Self::get(harness, "nobody.example.test", "/tea", &[]).await;
        assert_eq!(unknown.status, Status::NotFound);
        Outcome::Held
    }

    async fn ranges<H: Harness>(harness: &H) -> Outcome {
        let seed = Self::tea();
        harness.publish(&seed).await.unwrap();
        let bytes = &seed.pages[0].bytes;
        let part = Self::get(harness, "tea.example.test", "/tea", &[("range", "bytes=2-5")]).await;
        assert_eq!(part.status, Status::Partial);
        assert_eq!(part.body, bytes[2..6]);
        assert_eq!(part.range, Some(format!("bytes 2-5/{}", bytes.len())));
        let past = Self::get(harness, "tea.example.test", "/tea", &[("range", "bytes=900-")]).await;
        assert_eq!(past.status, Status::RangeNotSatisfiable);
        Outcome::Held
    }

    async fn many<H: Harness>(harness: &H) -> Outcome {
        let pages: u32 = 130;
        let mut seed = Seed::at("many.example.test");
        for n in 0..pages {
            seed = seed.page(&format!("/p{n}"), format!("<p>page number {n}</p>"));
        }
        let manifest = harness.publish(&seed).await.unwrap();
        let found = harness.sites().resolve(&Self::host("many.example.test")).await.unwrap().unwrap();
        assert_eq!(found.deploy.manifest.id(), manifest.id());
        let held: BTreeMap<Hash, &Vec<u8>> =
            seed.pages.iter().map(|page| (Hash::of(&page.bytes), &page.bytes)).collect();
        assert_eq!(found.deploy.manifest.entries().len(), pages as usize);
        for entry in found.deploy.manifest.entries() {
            let bytes = held[&entry.blob()];
            let blob = harness.blobs().open(&found.scope, entry.blob()).await.unwrap();
            assert_eq!(Self::drain(blob, Span::whole(bytes.len() as u64)).await, *bytes);
        }
        let last = Route::parse("/p129").unwrap();
        assert!(found.deploy.manifest.entry(&last).is_some());
        Outcome::Held
    }

    async fn suspends<H: Harness>(harness: &H) -> Outcome {
        if !harness.offers().suspensions {
            return Outcome::NotOffered("the adapter has no suspended sites");
        }
        let seed = Self::tea();
        harness.publish(&seed).await.unwrap();
        harness.suspend("tea.example.test").await.unwrap();
        assert!(harness.sites().resolve(&Self::host("tea.example.test")).await.unwrap().is_none());
        let notice = harness.sites().suspended(&Self::host("tea.example.test")).await.unwrap().unwrap();
        assert_ne!(notice.reason, "");
        let page = Self::get(harness, "tea.example.test", "/tea", &[]).await;
        assert_eq!(page.status, Status::Forbidden);
        let text = String::from_utf8(page.body).unwrap();
        assert!(text.contains("suspended"), "{text}");
        assert!(!text.contains("a pot of tea"), "{text}");
        Outcome::Held
    }

    async fn aliases<H: Harness>(harness: &H) -> Outcome {
        if !harness.offers().domains {
            return Outcome::NotOffered("the adapter has no custom domains");
        }
        let seed = Self::tea();
        let manifest = harness.publish(&seed).await.unwrap();
        harness.alias("tea.example.test", "www.example.org").await.unwrap();
        let found = harness.sites().resolve(&Self::host("www.example.org")).await.unwrap().unwrap();
        assert_eq!(found.deploy.manifest.id(), manifest.id());
        let page = Self::get(harness, "www.example.org", "/tea", &[]).await;
        assert_eq!((page.status, page.body), (Status::Ok, seed.pages[0].bytes.clone()));
        assert!(harness.sites().resolve(&Self::host("other.example.org")).await.unwrap().is_none());
        Outcome::Held
    }
}

/// What a `GET` through the edge answered, with its body read.
struct Got {
    status: Status,
    length: Option<String>,
    range: Option<String>,
    body: Vec<u8>,
}
