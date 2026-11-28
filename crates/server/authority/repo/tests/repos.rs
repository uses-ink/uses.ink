//! A repository claimed, proved, rotated and released on a control plane over real adapters and a
//! fake forge: the challenge file proves it, a repository only one site holds, and the secret is
//! shown once and checked later.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_authority_repo as authority_repo;

use std::sync::Arc;

use authority_forge::{Fake, Host, Limits, Method, Repo, Response, Unproven};
use authority_forgejo::Instances;
use authority_repo::{Error, Forges, Repos};
use blob_dir::Directory;
use build::Source;
use clock::Counter;
use control::{AccountKind, Control, Domain, Id, NewAccount, NewSite, Origin, Refusal, Remembered, SiteKind, Treed};
use db::Steps;
use db_sqlite::Sqlite;
use jiff::Timestamp;
use queue_rows::{Local as Queue, Queued};
use secret_local::Local as Sealer;

type Wired = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);

const FILE: &str = "https://codeberg.org/api/v1/repos/a/b/raw/.well-known/uses-ink";

fn at(n: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + n).unwrap()
}

struct World {
    control: Control<Wired>,
    repos: Repos<Fake>,
    draw: Counter,
    site: Id<SiteKind>,
    repo: Repo,
    _folder: tempfile::TempDir,
}

impl World {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
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
        let ports = (
            db.clone(),
            Directory::open(folder.path().join("blobs")).unwrap(),
            Queue::new(db),
            Sealer::ephemeral().unwrap(),
        );
        let origin = Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let control = Control::new(ports, origin);
        let account: Id<AccountKind> = Id::from_bytes([1; 16]);
        pollster::block_on(control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
        let source = Source::Git { url: "https://codeberg.org/a/b".parse().unwrap(), revision: None };
        let site = Id::from_bytes([2; 16]);
        let new = NewSite { id: site, account, handle: "journal".parse().unwrap(), source, at: at(1) };
        pollster::block_on(control.add_site(new)).unwrap();
        let forges =
            Forges { forgejo: Instances::bundled().unwrap(), gitlab: vec![Host::GITLAB], limits: Limits::DEFAULT };
        Self {
            control,
            repos: Repos::new(forges, Fake::new()),
            draw: Counter::new(),
            site,
            repo: "codeberg.org/a/b".parse().unwrap(),
            _folder: folder,
        }
    }

    fn ask(&self) -> (Id<SiteKind>, Repo, Timestamp) {
        (self.site, self.repo.clone(), at(10))
    }
}

#[test]
fn a_claim_shows_its_secret_once_and_a_proof_binds_the_repository() {
    let world = World::new();
    let claimed = pollster::block_on(world.repos.claim(&world.control, &world.draw, world.ask())).unwrap();
    assert!(!claimed.claim.bound());
    let shown = claimed.secret.text().unwrap().to_owned();
    assert_eq!(shown.len(), 64);
    assert!(!format!("{claimed:?}").contains(&shown), "the secret does not print");
    // the secret that was shown is the one sealed
    let sealed = pollster::block_on(world.control.hook_secret(&claimed.claim)).unwrap();
    assert_eq!(sealed.expose(), shown.as_bytes());
    // no file yet: not proved, and nothing serves
    world.repos.transport().reply(Method::Get, FILE, Response::new(404, "{}"));
    let failed = pollster::block_on(world.repos.prove(&world.control, world.ask())).unwrap_err();
    assert!(matches!(failed, Error::Unproven(Unproven::Absent)), "{failed:?}");
    let fake = world.repos.transport();
    fake.reply(Method::Get, FILE, Response::new(200, format!("# the file\n{}\n", claimed.claim.challenge)));
    let proved = pollster::block_on(world.repos.prove(&world.control, world.ask())).unwrap();
    assert!(proved.bound());
    assert_eq!(pollster::block_on(world.control.bound(&world.repo)).unwrap().unwrap().site, world.site);
    // a file with another challenge does not prove, a proved claim proves again
    fake.reply(Method::Get, FILE, Response::new(200, "uses-ink-repo=00000000000000000000000000000000\n"));
    let again = pollster::block_on(world.repos.prove(&world.control, world.ask()));
    assert!(matches!(again, Err(Error::Unproven(Unproven::Wrong))), "{again:?}");
}

#[test]
fn rotation_replaces_the_secret_and_release_removes_the_claim() {
    let world = World::new();
    let first = pollster::block_on(world.repos.claim(&world.control, &world.draw, world.ask())).unwrap();
    let second = pollster::block_on(world.repos.rotate(&world.control, &world.draw, world.ask())).unwrap();
    assert_ne!(first.secret.text(), second.secret.text());
    let held = pollster::block_on(world.control.hook_secret(&second.claim)).unwrap();
    assert_eq!(held.expose(), second.secret.text().unwrap().as_bytes());
    assert!(pollster::block_on(world.control.hook_secret(&first.claim)).is_err(), "the old secret is gone");
    pollster::block_on(world.repos.release(&world.control, world.ask())).unwrap();
    assert_eq!(pollster::block_on(world.control.repos(world.site)).unwrap().len(), 0);
    let failed = pollster::block_on(world.repos.release(&world.control, world.ask())).unwrap_err();
    match failed {
        Error::Control(error) => assert!(matches!(error.refusal(), Some(Refusal::NoRepo { .. }))),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_host_no_forge_lists_is_refused_before_anything_is_sent() {
    let world = World::new();
    let stranger: Repo = "evil.example/a/b".parse().unwrap();
    let challenge = authority_forge::Challenge::from_bytes([1; 16]);
    let failed = pollster::block_on(world.repos.read(&stranger, &challenge)).unwrap_err();
    assert!(matches!(failed, Unproven::Unlisted { .. }), "{failed:?}");
    assert!(world.repos.transport().taken().is_empty());
}
