//! The control plane's deploys, served by host: the live deploy of a handle, with the media types
//! and headers its manifest holds, and what a rollback, a removal and the time to live do to it.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_edge_control as edge_control;

use std::sync::Arc;
use std::time::Duration;

use blob::{Blob as _, Tenant};
use blob_dir::Directory;
use build::Source;
use clock::{Clock as _, Manual};
use control::{
    AccountKind, Control, Domain, Handle, Id, NewAccount, NewDeploy, NewSite, Origin, Ports as _, Remembered, SiteKind,
    Treed,
};
use db::Steps;
use db_sqlite::Sqlite;
use edge::{Authority, Blob as _, Blobs, Chunks as _, Edge, Limits, Name, Request, Scheme, Sites, Span, Status};
use edge_control::{Back, Fallback, Fronts, Settings};
use edge_folder::Files;
use jiff::Timestamp;
use model::{Hash, Header, HeaderName, HeaderValue, Manifest, MediaType, Route, Served};
use queue_rows::{Local as Queue, Queued};
use secret_local::Local as Sealer;

type Wired = Arc<(Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer)>;

/// A control plane, a second one over the same ports for the lookups, and its folder.
struct World {
    control: Control<Wired>,
    back: Option<Back<Wired, Arc<Manual>, Directory>>,
    fronts: Option<Fronts>,
    clock: Arc<Manual>,
    _folder: tempfile::TempDir,
}

fn start() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

fn block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

fn host(name: &str) -> edge::Hostname {
    Authority::try_from(name.as_bytes()).unwrap().hostname
}

impl World {
    fn new(settings: Settings) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("blobs");
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = control::Schema::after(
            Steps::new()
                .with::<Domain>()
                .unwrap()
                .with::<Remembered>()
                .unwrap()
                .with::<Treed>()
                .unwrap()
                .with::<Queued>()
                .unwrap(),
        )
        .unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports: Wired =
            Arc::new((db.clone(), Directory::open(&root).unwrap(), Queue::new(db), Sealer::ephemeral().unwrap()));
        let origin = Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let clock = Arc::new(Manual::new(start(), 0));
        let control = Control::new(Arc::clone(&ports), origin.clone());
        let lookups = Control::new(ports, origin);
        let (fronts, back) = Back::pair(lookups, Arc::clone(&clock), Directory::open(&root).unwrap(), settings);
        Self { control, back: Some(back), fronts: Some(fronts), clock, _folder: folder }
    }

    fn site(&self, handle: &str) -> Id<SiteKind> {
        let account: Id<AccountKind> = Id::from_bytes([1; 16]);
        pollster::block_on(self.control.add_account(NewAccount { id: account, at: start() })).ok();
        let id = Id::from_bytes([2; 16]);
        let source = Source::Folder("/srv/blog".into());
        let new = NewSite { id, account, handle: handle.parse::<Handle>().unwrap(), source, at: start() };
        pollster::block_on(self.control.add_site(new)).unwrap();
        id
    }

    /// Deploys one page, `/tea`, with this body and an own header, as the live deploy.
    fn publish(&self, site: Id<SiteKind>, body: &str, at: i64) -> model::DeployId {
        let tenant = Tenant::try_from(site).unwrap();
        let hash = Hash::of(body);
        pollster::block_on(self.control.ports().blob().put(&tenant, hash, [body.as_bytes()])).unwrap();
        let policy = Header::new(HeaderName::ReferrerPolicy, HeaderValue::parse("no-referrer").unwrap());
        let served = Served::new(Route::parse("/tea").unwrap(), hash, MediaType::Html, vec![policy]);
        let manifest = Manifest::new(vec![served]).unwrap();
        let at = Timestamp::from_second(start().as_second() + at).unwrap();
        pollster::block_on(self.control.deploy(NewDeploy { site, manifest, build: None, at })).unwrap().deploy
    }

    /// Runs `check` beside the lookups, then stops them.
    fn run<F: std::future::Future>(&mut self, check: impl FnOnce(Fronts) -> F) -> F::Output {
        let back = self.back.take().unwrap();
        let fronts = self.fronts.take().unwrap();
        block(async {
            let ((), output) = tokio::join!(back.run(), check(fronts));
            output
        })
    }
}

/// The bytes the deploy serves for `hash` in `scope`.
async fn read(scope: &<Files as Blobs>::Scope, hash: Hash) -> Vec<u8> {
    let blob = Files.open(scope, hash).await.unwrap();
    let mut chunks = blob.read(Span { start: 0, length: 4 });
    let mut bytes = Vec::new();
    while let Some(chunk) = chunks.next().await.unwrap() {
        bytes.extend_from_slice(&chunk);
    }
    bytes
}

#[test]
fn serves_the_live_deploy_of_a_handle_with_its_media_type_and_headers() {
    let mut world = World::new(Settings::default());
    let site = world.site("tea");
    let deploy = world.publish(site, "<h1>tea</h1>", 1);
    world.run(|fronts| async move {
        let resolved = fronts.resolve(&host("tea.uses.ink")).await.unwrap().unwrap();
        assert_eq!(resolved.deploy.manifest.id(), deploy);
        let entry = resolved.deploy.manifest.entry(&Route::parse("/tea").unwrap()).unwrap();
        assert_eq!(entry.media(), MediaType::Html);
        assert_eq!(entry.headers().len(), 1);
        assert_eq!(read(&resolved.scope, entry.blob()).await, b"<h1>");
        let edge = Edge::new(fronts.clone(), Files, Limits::default()).unwrap();
        let mut asked = Request::new("GET", "/tea", Scheme::Plain);
        asked.extend([(&b"host"[..], &b"tea.uses.ink"[..])]);
        let answer = edge.respond(&asked).await;
        assert_eq!(answer.response.status, Status::Ok);
        let kind = answer.response.field(Name::ContentType).unwrap();
        assert_eq!(kind.as_str(), "text/html; charset=utf-8");
        assert!(answer.response.field(Name::Entry(HeaderName::ReferrerPolicy)).is_some());
        let mut missing = Request::new("GET", "/", Scheme::Plain);
        missing.extend([(&b"host"[..], &b"tea.uses.ink"[..])]);
        assert_eq!(edge.respond(&missing).await.response.status, Status::NotFound);
    });
}

#[test]
fn a_host_that_is_no_site_is_no_site() {
    let mut world = World::new(Settings::default());
    world.site("tea");
    world.run(|fronts| async move {
        // a site with nothing live
        assert!(fronts.resolve(&host("tea.uses.ink")).await.unwrap().is_none());
        for other in ["coffee.uses.ink", "uses.ink", "tea.example.com", "a.tea.uses.ink", "127.0.0.1"] {
            assert!(fronts.resolve(&host(other)).await.unwrap().is_none(), "{other}");
        }
    });
}

#[test]
fn a_rollback_and_a_removal_are_seen_when_nothing_is_kept() {
    let mut world = World::new(Settings { ttl: Duration::ZERO, ..Settings::default() });
    let site = world.site("tea");
    let first = world.publish(site, "first", 1);
    let second = world.publish(site, "second", 2);
    let back = world.back.take().unwrap();
    let fronts = world.fronts.take().unwrap();
    let control = &world.control;
    block(async move {
        let lookups = async move {
            let live =
                || async { fronts.resolve(&host("tea.uses.ink")).await.unwrap().map(|r| r.deploy.manifest.id()) };
            assert_eq!(live().await, Some(second));
            control.rollback(site, first, start()).await.unwrap();
            assert_eq!(live().await, Some(first));
            control.remove_site(site, start()).await.unwrap();
            assert_eq!(live().await, None);
        };
        tokio::join!(back.run(), lookups);
    });
}

#[test]
fn a_pointer_is_believed_for_its_time_to_live_and_no_longer() {
    let mut world = World::new(Settings::default());
    let site = world.site("tea");
    let first = world.publish(site, "first", 1);
    let back = world.back.take().unwrap();
    let fronts = world.fronts.take().unwrap();
    let (control, clock) = (&world.control, Arc::clone(&world.clock));
    block(async move {
        let lookups = async move {
            let live = || async { fronts.resolve(&host("tea.uses.ink")).await.unwrap().unwrap().deploy.manifest.id() };
            assert_eq!(live().await, first);
            let tenant = Tenant::try_from(site).unwrap();
            let hash = Hash::of("second");
            control.ports().blob().put(&tenant, hash, ["second".as_bytes()]).await.unwrap();
            let served = Served::new(Route::parse("/tea").unwrap(), hash, MediaType::Html, vec![]);
            let manifest = Manifest::new(vec![served]).unwrap();
            let new = NewDeploy { site, manifest, build: None, at: clock.now() };
            let second = control.deploy(new).await.unwrap().deploy;
            assert_eq!(live().await, first, "the old pointer is believed for the time to live");
            clock.advance(Settings::TTL);
            assert_eq!(live().await, second);
        };
        tokio::join!(back.run(), lookups);
    });
}

#[test]
fn a_folder_is_asked_when_the_control_plane_has_no_site() {
    let mut world = World::new(Settings::default());
    let site = world.site("tea");
    world.publish(site, "stored", 1);
    let folder = tempfile::tempdir().unwrap();
    let deploys = edge_folder::Deploys::new(folder.path(), edge_folder::Mode::Folders).unwrap();
    world.run(|fronts| async move {
        let both = Fallback::new(Some(fronts), deploys);
        assert!(both.resolve(&host("tea.uses.ink")).await.unwrap().is_some());
        assert!(both.resolve(&host("other.example")).await.unwrap().is_none());
    });
}

#[test]
fn a_suspended_site_answers_the_notice_and_none_of_its_bytes() {
    let mut world = World::new(Settings { ttl: Duration::ZERO, ..Settings::default() });
    let site = world.site("tea");
    world.publish(site, "<h1>tea</h1>", 1);
    let suspension = control::Suspension {
        site,
        kind: control::AbuseKind::Malware,
        by: "operator".to_owned(),
        report: None,
        at: start(),
    };
    pollster::block_on(world.control.suspend(suspension)).unwrap();
    world.run(|fronts| async move {
        let edge = Edge::new(fronts.clone(), Files, Limits::default()).unwrap();
        let mut asked = Request::new("GET", "/tea", Scheme::Plain);
        asked.extend([(&b"host"[..], &b"tea.uses.ink"[..])]);
        let answer = edge.respond(&asked).await;
        assert_eq!(answer.response.status, Status::Forbidden);
        assert!(matches!(answer.response.body, edge::Body::Text(ref text) if text.contains("suspended for malware") && !text.contains("tea<")));
    });
}

#[test]
fn a_preview_is_served_at_its_label_until_it_ends() {
    let mut world = World::new(Settings::default());
    let account: Id<AccountKind> = Id::from_bytes([1; 16]);
    pollster::block_on(world.control.add_account(NewAccount { id: account, at: start() })).unwrap();
    let source = Source::Git { url: "https://codeberg.org/cstef/tea".parse().unwrap(), revision: None };
    let id = Id::from_bytes([2; 16]);
    let new = NewSite { id, account, handle: "tea".parse::<Handle>().unwrap(), source, at: start() };
    pollster::block_on(world.control.add_site(new)).unwrap();
    // the site has nothing live; only its preview serves
    let tenant = Tenant::try_from(id).unwrap();
    let hash = Hash::of("<h1>branch</h1>");
    pollster::block_on(world.control.ports().blob().put(&tenant, hash, [b"<h1>branch</h1>".as_slice()])).unwrap();
    let served = Served::new(Route::parse("/tea").unwrap(), hash, MediaType::Html, vec![]);
    let manifest = Manifest::new(vec![served]).unwrap();
    let new = control::NewPreview { site: id, slug: "main".parse().unwrap(), manifest: manifest.clone(), at: start() };
    pollster::block_on(world.control.preview(new)).unwrap();
    let clock = Arc::clone(&world.clock);
    world.run(|fronts| async move {
        let resolved = fronts.resolve(&host("main--tea.uses.ink")).await.unwrap().unwrap();
        assert_eq!(resolved.deploy.manifest.id(), manifest.id());
        let entry = resolved.deploy.manifest.entry(&Route::parse("/tea").unwrap()).unwrap();
        assert_eq!(read(&resolved.scope, entry.blob()).await, b"<h1>");
        // the preview is no site, and a label nobody pushed serves nothing
        assert!(fronts.resolve(&host("tea.uses.ink")).await.unwrap().is_none());
        assert!(fronts.resolve(&host("other--tea.uses.ink")).await.unwrap().is_none());
        assert!(fronts.resolve(&host("main--nobody.uses.ink")).await.unwrap().is_none());
        // seven days and a moment later it has ended, with no cache to wait out
        clock.advance(Duration::from_secs(7 * 24 * 3600 + 1));
        assert!(fronts.resolve(&host("main--tea.uses.ink")).await.unwrap().is_none());
    });
}

impl World {
    /// Makes `name` serve `site`, as the DNS authority's proof would: a claim, then a record that
    /// carries its token.
    fn domain(&self, site: Id<SiteKind>, name: &str) {
        let name: control::DomainName = name.parse().unwrap();
        let token = control::Token::from_bytes([7; 16]);
        pollster::block_on(self.control.claim_domain(control::NewClaim {
            site,
            name: name.clone(),
            token,
            at: start(),
        }))
        .unwrap();
        let found = control::Found::new(1, vec![token]);
        pollster::block_on(self.control.prove_domain(control::Proving { site, name, found, at: start() })).unwrap();
    }
}

#[test]
fn a_proven_custom_domain_serves_the_site_until_it_is_removed() {
    let mut world = World::new(Settings::default());
    let site = world.site("tea");
    let deploy = world.publish(site, "<h1>tea</h1>", 1);
    world.domain(site, "www.example.com");
    let name: control::DomainName = "pending.example.com".parse().unwrap();
    let claim = control::NewClaim { site, name, token: control::Token::from_bytes([8; 16]), at: start() };
    pollster::block_on(world.control.claim_domain(claim)).unwrap();
    let lookups = Control::new(Arc::clone(world.control.ports()), world.control.origin().clone());
    world.run(|fronts| async move {
        let resolved = fronts.resolve(&host("www.example.com")).await.unwrap().unwrap();
        assert_eq!(resolved.deploy.manifest.id(), deploy);
        // a name that was only claimed, one nobody claimed and an address serve nothing
        assert!(fronts.resolve(&host("pending.example.com")).await.unwrap().is_none());
        assert!(fronts.resolve(&host("other.example.com")).await.unwrap().is_none());
        assert!(fronts.resolve(&host("127.0.0.1")).await.unwrap().is_none());
        // the handle's own address still serves, and the domain stops the moment it is removed
        assert!(fronts.resolve(&host("tea.uses.ink")).await.unwrap().is_some());
        lookups.remove_domain(site, &"www.example.com".parse().unwrap(), start()).await.unwrap();
        assert!(fronts.resolve(&host("www.example.com")).await.unwrap().is_none());
        assert!(fronts.resolve(&host("tea.uses.ink")).await.unwrap().is_some());
    });
}

#[test]
fn a_suspended_site_answers_the_notice_at_its_custom_domain_too() {
    let mut world = World::new(Settings { ttl: Duration::ZERO, ..Settings::default() });
    let site = world.site("tea");
    world.publish(site, "<h1>tea</h1>", 1);
    world.domain(site, "www.example.com");
    let suspension = control::Suspension {
        site,
        kind: control::AbuseKind::Malware,
        by: "operator".to_owned(),
        report: None,
        at: start(),
    };
    pollster::block_on(world.control.suspend(suspension)).unwrap();
    world.run(|fronts| async move {
        let edge = Edge::new(fronts.clone(), Files, Limits::default()).unwrap();
        let mut asked = Request::new("GET", "/tea", Scheme::Plain);
        asked.extend([(&b"host"[..], &b"www.example.com"[..])]);
        let answer = edge.respond(&asked).await;
        assert_eq!(answer.response.status, Status::Forbidden);
        assert!(matches!(answer.response.body, edge::Body::Text(ref text) if text.contains("suspended for malware") && !text.contains("tea<")));
    });
}

#[test]
fn a_removed_site_frees_its_custom_domain_at_once() {
    let mut world = World::new(Settings::default());
    let site = world.site("tea");
    world.publish(site, "<h1>tea</h1>", 1);
    world.domain(site, "www.example.com");
    let lookups = Control::new(Arc::clone(world.control.ports()), world.control.origin().clone());
    world.run(|fronts| async move {
        assert!(fronts.resolve(&host("www.example.com")).await.unwrap().is_some());
        lookups.remove_site(site, start()).await.unwrap();
        assert!(fronts.resolve(&host("www.example.com")).await.unwrap().is_none());
    });
}
