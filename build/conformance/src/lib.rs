//! The conformance suite of the build port: each case is a type with a `check` over any
//! [`Build`], and [`suite!`] turns every case into a test. An adapter supplies a [`Harness`]:
//! the builder, a read handle on the blobs it writes, a source that builds and one that does not.
//!
//! ```ignore
//! build_conformance::suite!(|| (MyHarness::new(), ()));
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use blob::{Blob, Chunks, Tenant};
use build::{Build, BuildId, Event, Outcome, Request, Source, Status, Trigger};
use model::{ExternalUrl, Hash};

#[doc(hidden)]
pub use pollster;

/// What an adapter gives the suite.
#[derive(Debug)]
pub struct Harness<B, R> {
    /// The builder under test.
    pub builder: B,
    /// A way to read the blobs the builder stores.
    pub blobs: R,
    /// A source that builds into a site with at least one file.
    pub good: Source,
    /// A source that does not build: the site has an error.
    pub broken: Source,
}

impl<B: Build, R: Blob> Harness<B, R> {
    /// A request for `source` under the tenant `tenant`.
    fn request(source: &Source, tenant: &str, n: u8) -> Request {
        Request {
            build: BuildId::from_bytes([n; 16]),
            tenant: tenant.parse::<Tenant>().unwrap(),
            trigger: Trigger::Manual,
            source: source.clone(),
            base: ExternalUrl::parse("https://cstef.uses.ink").unwrap(),
        }
    }

    /// Builds `source`, returning the outcome and every event.
    async fn build(&self, source: &Source, tenant: &str) -> (Outcome, Vec<Event>) {
        let mut events = Vec::new();
        let outcome = self.builder.run(&Self::request(source, tenant, 1), &mut events).await.unwrap();
        (outcome, events)
    }
}

/// A build that works.
#[derive(Debug, Clone, Copy)]
pub struct Success;

impl Success {
    /// The stream starts and ends as it must, and every blob the manifest names is stored and
    /// reads back as its hash.
    pub async fn check<B: Build, R: Blob>(harness: &Harness<B, R>) {
        let (outcome, events) = harness.build(&harness.good, "site-a").await;
        let Outcome::Built(built) = outcome else { panic!("the good source did not build: {events:#?}") };
        assert!(matches!(events.first(), Some(Event::Started { .. })), "{events:#?}");
        assert!(matches!(events.last(), Some(Event::Finished { status: Status::Succeeded, .. })), "{events:#?}");
        assert!(events.iter().any(|event| matches!(event, Event::SourceFetched { .. })));
        assert!(events.iter().any(|event| matches!(event, Event::Rendered { .. })));
        assert!(!built.manifest.entries().is_empty());
        let tenant: Tenant = "site-a".parse().unwrap();
        for entry in built.manifest.entries() {
            let read = harness.blobs.get(&tenant, entry.blob()).await.unwrap();
            let bytes = read.unwrap_or_else(|| panic!("{} is not stored", entry.route())).all().await.unwrap();
            assert_eq!(Hash::of(bytes), entry.blob());
        }
    }
}

/// The same source gives the same deploy.
#[derive(Debug, Clone, Copy)]
pub struct Repeatable;

impl Repeatable {
    /// Two builds, even under two tenants, make one manifest.
    pub async fn check<B: Build, R: Blob>(harness: &Harness<B, R>) {
        let (first, _) = harness.build(&harness.good, "site-a").await;
        let (second, _) = harness.build(&harness.good, "site-b").await;
        let (Outcome::Built(first), Outcome::Built(second)) = (first, second) else { panic!("a build was refused") };
        assert_eq!(first.manifest.id(), second.manifest.id());
    }
}

/// A site that does not build.
#[derive(Debug, Clone, Copy)]
pub struct Refusal;

impl Refusal {
    /// It is refused, not an error: the stream says why and ends as refused, and nothing is
    /// stored.
    pub async fn check<B: Build, R: Blob>(harness: &Harness<B, R>) {
        let (outcome, events) = harness.build(&harness.broken, "site-a").await;
        assert!(matches!(outcome, Outcome::Refused), "{outcome:?}");
        assert!(matches!(events.first(), Some(Event::Started { .. })));
        assert!(matches!(events.last(), Some(Event::Finished { status: Status::Refused, .. })), "{events:#?}");
        assert!(
            events.iter().any(|event| matches!(event, Event::Diagnosed(_) | Event::Refused(_))),
            "refused without saying why: {events:#?}"
        );
        let tenant: Tenant = "site-a".parse().unwrap();
        assert!(harness.blobs.list(&tenant).await.unwrap().is_empty(), "a refused build stored blobs");
    }
}

/// Blobs are the tenant's.
#[derive(Debug, Clone, Copy)]
pub struct Tenants;

impl Tenants {
    /// A build under one tenant leaves nothing under another.
    pub async fn check<B: Build, R: Blob>(harness: &Harness<B, R>) {
        harness.build(&harness.good, "site-a").await;
        let other: Tenant = "site-b".parse().unwrap();
        assert!(harness.blobs.list(&other).await.unwrap().is_empty());
    }
}

/// Runs every case of the build port's suite against the harness `$make` builds: a closure that
/// returns a [`Harness`] and whatever must outlive it (a temporary folder, or `()`) as a pair.
/// Each case is a test of its own, so each has its own budget.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::suite!(@case $make, builds_and_stores_what_it_names, Success);
        $crate::suite!(@case $make, builds_the_same_site_to_the_same_deploy, Repeatable);
        $crate::suite!(@case $make, refuses_a_site_that_does_not_build_and_says_why, Refusal);
        $crate::suite!(@case $make, leaves_nothing_under_another_tenant, Tenants);
    };
    (@case $make:expr, $name:ident, $case:ident) => {
        #[test]
        fn $name() {
            let (harness, _guard) = ($make)();
            $crate::pollster::block_on($crate::$case::check(&harness));
        }
    };
}
