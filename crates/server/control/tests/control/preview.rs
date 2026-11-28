//! Previews of branches: unlisted, expiring, swept, and never a site's own address.

use std::time::Duration;

use blob::{Blob, Tenant};
use build::Source;
use control::table::{Deploy, Preview as Rows};
use control::{
    AccountKind, Limits, NewAccount, NewDeploy, NewPreview, NewSite, Origin, Ports as _, Purpose, Refusal, Site,
    SiteKind, Slug, SlugError,
};
use model::{Bytes, ExternalUrl, Hash};
use secret::Plain;

use crate::world::{World, at};

/// A preview lasts a thousand seconds in these tests.
const LIFE: Duration = Duration::from_secs(1000);

fn world() -> World {
    World::new().under(World::BARE.previewing(LIFE))
}

/// A site of a new account with a public git source.
fn repo(world: &World, handle: &str) -> Site {
    let account: control::Id<AccountKind> = world.id();
    world.run(world.control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
    world
        .run(world.control.add_site(NewSite {
            id: world.id::<SiteKind>(),
            account,
            handle: handle.parse().unwrap(),
            source: Source::Git { url: "https://codeberg.org/cstef/blog".parse().unwrap(), revision: None },
            at: at(1),
        }))
        .unwrap()
}

fn slug(text: &str) -> Slug {
    text.parse().unwrap()
}

fn preview(world: &World, site: &Site, when: (&str, i64), pages: &[(&str, &str)]) -> NewPreview {
    NewPreview { site: site.id, slug: slug(when.0), manifest: world.stage(site, pages), at: at(when.1) }
}

fn blobs(world: &World, site: &Site) -> Vec<Hash> {
    let tenant = Tenant::try_from(site.id).unwrap();
    world.run(world.control.ports().blob().list(&tenant)).unwrap().iter().map(|listed| listed.hash).collect()
}

#[test]
fn a_preview_serves_at_its_own_label_and_touches_no_deploy() {
    let world = world();
    let site = repo(&world, "cstef");
    let live = world.stage(&site, &[("/", "live")]);
    world
        .run(world.control.deploy(NewDeploy { site: site.id, manifest: live.clone(), build: None, at: at(10) }))
        .unwrap();
    let new = preview(&world, &site, ("new-design", 20), &[("/", "branch")]);
    let made = world.run(world.control.preview(new.clone())).unwrap();
    assert_eq!((made.deploy, made.files, made.expires), (new.manifest.id(), 1, at(1020)));
    let showing = world.run(world.control.previewed((&slug("new-design"), &site.handle), at(30))).unwrap().unwrap();
    assert_eq!((showing.site, showing.deploy), (site.id, new.manifest.id()));
    // the site still serves its own deploy, and no list of deploys names the preview
    assert_eq!(world.run(world.control.live(site.id)).unwrap().unwrap().deploy, live.id());
    let listed = world.run(world.control.deploys(site.id, 10)).unwrap();
    assert_eq!(listed.iter().map(|d| d.id).collect::<Vec<_>>(), [live.id()]);
    assert_eq!(world.count(Deploy::Table), 1);
    // another label, and another site's handle, serve nothing
    assert!(world.run(world.control.previewed((&slug("other"), &site.handle), at(30))).unwrap().is_none());
    assert!(
        world
            .run(world.control.previewed((&slug("new-design"), &"nobody".parse().unwrap()), at(30)))
            .unwrap()
            .is_none()
    );
    assert_eq!(world.events("preview.pushed"), 1);
    assert_eq!(world.run(world.control.previews(site.id, at(30))).unwrap().len(), 1);
}

#[test]
fn a_preview_ends_and_its_own_blobs_are_swept() {
    let world = world();
    let site = repo(&world, "cstef");
    let live = world.stage(&site, &[("/", "shared"), ("/a", "live only")]);
    world
        .run(world.control.deploy(NewDeploy { site: site.id, manifest: live.clone(), build: None, at: at(10) }))
        .unwrap();
    let new = preview(&world, &site, ("feature", 20), &[("/", "shared"), ("/b", "preview only")]);
    world.run(world.control.preview(new.clone())).unwrap();
    let before = blobs(&world, &site);
    assert!(before.contains(&Hash::of("preview only")));
    // still there one tick before the end, gone at the end
    assert!(world.run(world.control.previewed((&slug("feature"), &site.handle), at(1019))).unwrap().is_some());
    assert!(world.run(world.control.previewed((&slug("feature"), &site.handle), at(1020))).unwrap().is_none());
    assert!(world.run(world.control.previews(site.id, at(1020))).unwrap().is_empty());
    let early = world.run(world.control.expire(at(1019))).unwrap();
    assert_eq!(early.previews, 0);
    let swept = world.run(world.control.expire(at(1020))).unwrap();
    assert_eq!(swept.previews, 1);
    // the preview's manifest and its own file go; the file the live deploy names stays
    assert_eq!(swept.blobs, 2);
    assert_eq!(swept.bytes, Bytes(("preview only".len() + new.manifest.canonical().len()) as u64));
    let after = blobs(&world, &site);
    assert!(!after.contains(&Hash::of("preview only")) && !after.contains(&new.manifest.id().hash()));
    assert!(after.contains(&Hash::of("shared")) && after.contains(&live.id().hash()));
    assert_eq!(world.count(Rows::Table), 0);
    assert_eq!(world.events("preview.expired"), 1);
    assert_eq!(world.run(world.control.expire(at(2000))).unwrap().previews, 0, "nothing is swept twice");
}

#[test]
fn a_new_push_to_a_label_replaces_the_preview_and_frees_what_only_it_held() {
    let world = world();
    let site = repo(&world, "cstef");
    world.run(world.control.preview(preview(&world, &site, ("main", 10), &[("/", "one"), ("/x", "kept")]))).unwrap();
    let second = preview(&world, &site, ("main", 500), &[("/", "two"), ("/x", "kept")]);
    let made = world.run(world.control.preview(second.clone())).unwrap();
    assert_eq!(made.expires, at(1500), "a push starts the time over");
    assert_eq!(world.count(Rows::Table), 1);
    let held = blobs(&world, &site);
    assert!(!held.contains(&Hash::of("one")), "the replaced preview's own file is freed");
    assert!(held.contains(&Hash::of("two")) && held.contains(&Hash::of("kept")));
}

#[test]
fn retention_never_sweeps_a_blob_a_preview_serves() {
    let world = World::new().under(World::BARE.previewing(LIFE).limited(Limits { deploys: 1, ..Limits::DEFAULT }));
    let site = repo(&world, "cstef");
    // the preview shares the file "a" with the oldest deploy, which is the one the sweep drops
    let new = preview(&world, &site, ("feature", 5), &[("/", "a"), ("/p", "only the preview has this")]);
    world.run(world.control.preview(new.clone())).unwrap();
    for (n, text) in [(10, "a"), (20, "b"), (30, "c")] {
        let manifest = world.stage(&site, &[("/", text)]);
        world.run(world.control.deploy(NewDeploy { site: site.id, manifest, build: None, at: at(n) })).unwrap();
    }
    assert_eq!(world.count(Deploy::Table), 2, "the live deploy and the one it replaced are kept, the oldest is swept");
    let held = blobs(&world, &site);
    assert!(held.contains(&Hash::of("only the preview has this")) && held.contains(&new.manifest.id().hash()));
    assert!(held.contains(&Hash::of("a")), "a file the dropped deploy shares with a preview stays");
    assert!(world.run(world.control.previewed((&slug("feature"), &site.handle), at(40))).unwrap().is_some());
}

#[test]
fn only_a_public_git_site_has_previews() {
    let world = world();
    let upload = world.site("folder");
    let refused = world.run(world.control.preview(preview(&world, &upload, ("main", 10), &[("/", "x")]))).unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refusal::NoPreview { site: upload.id }));
    assert_eq!(refused.refusal().map(Refusal::code), Some("preview.site.unsupported"));
    // a git token makes the repository private, and a private repository's branches are never served
    let private = repo(&world, "secret");
    world
        .run(world.control.add_credential(control::NewCredential {
            id: world.credential(),
            site: private.id,
            purpose: Purpose::GitToken,
            secret: Plain::from("token"),
            at: at(2),
        }))
        .unwrap();
    let refused = world.run(world.control.preview(preview(&world, &private, ("main", 10), &[("/", "x")]))).unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refusal::Private { site: private.id }));
    assert_eq!(refused.refusal().map(Refusal::code), Some("preview.site.private"));
    assert_eq!(world.count(Rows::Table), 0);
}

#[test]
fn a_preview_over_the_storage_budget_is_refused_whole() {
    let world = World::new().under(World::BARE.limited(Limits { stored: Bytes(300), ..Limits::DEFAULT }));
    let site = repo(&world, "cstef");
    let big = "b".repeat(400);
    let refused = world.run(world.control.preview(preview(&world, &site, ("main", 10), &[("/", big.as_str())])));
    // staging the files already put them in the store; the refusal takes them out again
    assert!(matches!(refused.unwrap_err().refusal(), Some(Refusal::Over { .. })));
    assert_eq!(world.count(Rows::Table), 0);
    assert!(blobs(&world, &site).is_empty());
}

#[test]
fn a_label_is_checked_and_a_preview_address_is_never_a_handle() {
    assert_eq!("a--b".parse::<Slug>(), Err(SlugError::Hyphen));
    assert_eq!("-a".parse::<Slug>(), Err(SlugError::Hyphen));
    assert_eq!("A".parse::<Slug>(), Err(SlugError::Character { at: 0 }));
    assert_eq!("".parse::<Slug>(), Err(SlugError::Empty));
    assert_eq!("x".repeat(30).parse::<Slug>(), Err(SlugError::Long { found: 30 }));
    assert_eq!(Slug::branch("feature/Login_v2").unwrap().as_str(), "feature-login-v2");
    assert_eq!(Slug::branch("--weird--").unwrap().as_str(), "weird");
    assert_eq!(Slug::branch("///"), Err(SlugError::Empty));
    let cut = Slug::branch(&"a-".repeat(40)).unwrap();
    assert!(cut.as_str().len() <= Slug::LONGEST && !cut.as_str().ends_with('-'), "{cut}");
    // however a label is made, its host is a preview and no site's host, even for a label that is another site's handle
    let origin = Origin::try_from(&ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
    for label in ["main", "www", "github", "diary", "a-b"] {
        let slug = slug(label);
        let owner = "cstef".parse().unwrap();
        let host = origin.previewed(&slug, &owner).unwrap();
        let name = host.as_str().strip_prefix("https://").unwrap();
        assert_eq!(origin.handle(name), None, "{name} is no site");
        assert_eq!(origin.preview(name), Some((slug, owner)));
    }
}

#[test]
fn removing_a_site_removes_its_previews() {
    let world = world();
    let site = repo(&world, "cstef");
    world.run(world.control.preview(preview(&world, &site, ("main", 10), &[("/", "x")]))).unwrap();
    world.run(world.control.remove_site(site.id, at(20))).unwrap();
    assert_eq!(world.count(Rows::Table), 0);
    assert!(world.run(world.control.previewed((&slug("main"), &site.handle), at(21))).unwrap().is_none());
}

fn pin(branch: &str, digit: char) -> build::Pin {
    build::Pin { branch: branch.parse().unwrap(), revision: digit.to_string().repeat(40).parse().unwrap() }
}

fn branch_job(site: &Site, n: u8) -> control::NewJob {
    control::NewJob {
        id: build::BuildId::from_bytes([n; 16]),
        site: site.id,
        trigger: build::Trigger::Poll,
        dedupe: None,
        at: at(i64::from(n)),
    }
}

#[test]
fn a_branch_build_reads_its_commit_and_ends_as_a_preview_not_a_deploy() {
    use crate::job::{LEASE, Scripted, Step};
    let world = world();
    let site = repo(&world, "cstef");
    let pinned = pin("feature/login", 'c');
    assert!(!world.run(world.control.pinned(site.id, &pinned)).unwrap());
    let queued = world.run(world.control.request_branch(branch_job(&site, 10), pinned.clone())).unwrap();
    assert_eq!(queued, control::Requested::Queued(build::BuildId::from_bytes([10; 16])));
    assert!(world.run(world.control.pinned(site.id, &pinned)).unwrap(), "asked once, never again");
    assert!(!world.run(world.control.pinned(site.id, &pin("feature/login", 'd'))).unwrap(), "a moved branch is new");
    let job = world.run(world.control.job(build::BuildId::from_bytes([10; 16]))).unwrap().unwrap();
    assert_eq!(job.pin, Some(pinned.clone()));
    let builder = Scripted::new(&world, [Step::Build("branch")]);
    let worked = world.run(world.control.work(&builder, at(11), LEASE)).unwrap();
    let control::Worked::Previewed { slug: made, .. } = worked else { panic!("{worked:?}") };
    assert_eq!(made, slug("feature-login"));
    let seen = builder.seen().first().cloned().unwrap();
    assert_eq!(seen.pin, Some(pinned.clone()));
    let Source::Git { revision, .. } = seen.source else { panic!("{:?}", seen.source) };
    assert_eq!(revision, Some(pinned.branch.revision()), "the build reads the branch it was queued for");
    assert_eq!(world.run(world.control.live(site.id)).unwrap(), None, "no deploy was made");
    assert_eq!(world.run(world.control.previews(site.id, at(12))).unwrap().len(), 1);
}

#[test]
fn a_branch_that_makes_no_label_or_a_site_that_is_not_git_is_refused() {
    let world = world();
    let site = repo(&world, "cstef");
    let bad = world.run(world.control.request_branch(branch_job(&site, 10), pin("..", 'c'))).unwrap_err();
    assert!(matches!(bad, control::Error::Label(_)), "{bad:?}");
    let upload = world.site("garden");
    let refused = world.run(world.control.request_branch(branch_job(&upload, 11), pin("main", 'c'))).unwrap_err();
    assert!(matches!(refused, control::Error::Refused(Refusal::NoPreview { .. })), "{refused:?}");
}

#[test]
fn a_preview_is_removed_with_its_blobs_and_a_typo_is_not_a_success() {
    let world = world();
    let site = repo(&world, "cstef");
    world.run(world.control.preview(preview(&world, &site, ("draft", 20), &[("/", "draft")]))).unwrap();
    assert_eq!(blobs(&world, &site).len(), 2, "the file and the manifest");
    let gone = world.run(world.control.remove_preview(site.id, &slug("draft"), at(30))).unwrap();
    assert_eq!(gone.blobs, 2);
    assert!(blobs(&world, &site).is_empty());
    assert!(world.run(world.control.previewed((&slug("draft"), &site.handle), at(31))).unwrap().is_none());
    let again = world.run(world.control.remove_preview(site.id, &slug("draft"), at(32))).unwrap_err();
    assert!(matches!(again, control::Error::Refused(Refusal::NoLabel { .. })), "{again:?}");
    assert_eq!(world.events("preview.expired"), 1);
}
