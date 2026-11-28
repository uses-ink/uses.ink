//! Export.

use std::fmt;

use control::{Exported, Failed, NewDeploy, Refusal, Sink};
use model::Route;

use crate::world::{World, at};

#[derive(Default)]
struct Collect {
    files: Vec<Exported>,
    refuse_after: Option<usize>,
}

#[derive(Debug)]
struct Full;

impl fmt::Display for Full {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the sink is full")
    }
}

impl std::error::Error for Full {}

impl Sink for Collect {
    type Error = Full;

    #[allow(clippy::unused_async_trait_impl, reason = "the sink collects in memory and waits for nothing")]
    async fn file(&mut self, file: Exported) -> Result<(), Full> {
        if self.refuse_after.is_some_and(|n| self.files.len() >= n) {
            return Err(Full);
        }
        self.files.push(file);
        Ok(())
    }
}

#[test]
fn writes_every_file_of_the_live_deploy_in_route_order() {
    let world = World::new();
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/posts/b", "bee"), ("/", "home"), ("/posts/a", "ay")]);
    world
        .run(world.control.deploy(NewDeploy { site: site.id, manifest: manifest.clone(), build: None, at: at(10) }))
        .unwrap();
    let mut sink = Collect::default();
    let summary = world.run(world.control.export(site.id, None, &mut sink)).unwrap();
    let routes: Vec<String> = sink.files.iter().map(|f| f.route.to_string()).collect();
    assert_eq!(routes, ["/", "/posts/a", "/posts/b"]);
    assert_eq!(
        sink.files.iter().map(|f| f.bytes.clone()).collect::<Vec<_>>(),
        [b"home".to_vec(), b"ay".to_vec(), b"bee".to_vec()]
    );
    assert_eq!((summary.deploy, summary.files, summary.bytes), (manifest.id(), 3, model::Bytes(9)));
}

#[test]
fn exports_a_named_older_deploy() {
    let world = World::new();
    let site = world.site("cstef");
    let (one, two) = (world.stage(&site, &[("/", "one")]), world.stage(&site, &[("/", "two")]));
    for (manifest, n) in [(&one, 10), (&two, 20)] {
        world
            .run(world.control.deploy(NewDeploy { site: site.id, manifest: manifest.clone(), build: None, at: at(n) }))
            .unwrap();
    }
    let mut sink = Collect::default();
    world.run(world.control.export(site.id, Some(one.id()), &mut sink)).unwrap();
    assert_eq!(sink.files.first().unwrap().bytes, b"one");
    assert_eq!(sink.files.first().unwrap().route, Route::root());
}

#[test]
fn refuses_what_has_nothing_to_export() {
    let world = World::new();
    let site = world.site("cstef");
    let other = world.site("other");
    let foreign = world.stage(&other, &[("/", "x")]);
    let mut sink = Collect::default();
    let none = world.run(world.control.export(site.id, None, &mut sink)).unwrap_err();
    assert!(matches!(none, Failed::Control(ref e) if matches!(e.refusal(), Some(Refusal::NotLive { .. }))));
    let unknown = world.run(world.control.export(site.id, Some(foreign.id()), &mut sink)).unwrap_err();
    assert!(matches!(unknown, Failed::Control(ref e) if matches!(e.refusal(), Some(Refusal::NoDeploy { .. }))));
    assert!(sink.files.is_empty());
}

#[test]
fn a_failing_sink_stops_the_export_and_says_so() {
    let world = World::new();
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/", "a"), ("/b", "b")]);
    world.run(world.control.deploy(NewDeploy { site: site.id, manifest, build: None, at: at(10) })).unwrap();
    let mut sink = Collect { refuse_after: Some(1), ..Collect::default() };
    let failed = world.run(world.control.export(site.id, None, &mut sink)).unwrap_err();
    assert!(matches!(failed, Failed::Other(Full)));
    assert_eq!(sink.files.len(), 1);
}

#[test]
fn a_blob_changed_on_disk_fails_the_export_instead_of_exporting_wrong_bytes() {
    let world = World::new();
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/", "honest bytes")]);
    world
        .run(world.control.deploy(NewDeploy { site: site.id, manifest: manifest.clone(), build: None, at: at(10) }))
        .unwrap();
    let blob = manifest.entries().first().unwrap().blob();
    let file = world.root.join("t").join(site.id.to_string()).join("b").join(blob.to_string());
    std::fs::write(file, "forged bytes").unwrap();
    let mut sink = Collect::default();
    let failed = world.run(world.control.export(site.id, None, &mut sink)).unwrap_err();
    assert!(matches!(failed, Failed::Control(control::Error::Blob(blob::Error::Corrupt { .. }))), "{failed:?}");
}
