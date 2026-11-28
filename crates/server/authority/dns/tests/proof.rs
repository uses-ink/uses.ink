//! The DNS proof on a control plane over real adapters and a fake resolver: a proof binds a name to
//! the site that claimed it, a proven name cannot be taken, and a removed site or domain frees it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

use std::sync::Arc;

extern crate uses_ink_authority_dns as authority_dns;

use authority_dns::{Challenge, Dns, Error, Fake, Host};
use blob_dir::Directory;
use build::Source;
use control::{
    AccountKind, Control, Domain, DomainName, Id, NewAccount, NewClaim, NewSite, Origin, Refusal, Remembered, Site,
    SiteKind, Token, Treed,
};
use db::Steps;
use db_sqlite::Sqlite;
use jiff::Timestamp;
use queue_rows::{Local as Queue, Queued};
use secret_local::Local as Sealer;

type Wired = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);

struct World {
    control: Control<Wired>,
    dns: Dns<Fake>,
    next: std::cell::Cell<u8>,
    _folder: tempfile::TempDir,
}

fn at(n: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + n).unwrap()
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
        let blobs = Directory::open(folder.path().join("blobs")).unwrap();
        let origin = Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let ports = (db.clone(), blobs, Queue::new(db), Sealer::ephemeral().unwrap());
        Self {
            control: Control::new(ports, origin),
            dns: Dns::new(Fake::new()),
            next: std::cell::Cell::new(1),
            _folder: folder,
        }
    }

    fn id<K>(&self) -> Id<K> {
        let n = self.next.get();
        self.next.set(n + 1);
        Id::from_bytes([n; 16])
    }

    fn site(&self, handle: &str) -> Site {
        let account: Id<AccountKind> = self.id();
        pollster::block_on(self.control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
        let new = NewSite {
            id: self.id::<SiteKind>(),
            account,
            handle: handle.parse().unwrap(),
            source: Source::Folder("/srv/blog".into()),
            at: at(1),
        };
        pollster::block_on(self.control.add_site(new)).unwrap()
    }

    fn claim(&self, site: &Site, name: &str) -> Result<control::Claim, control::Error<Wired>> {
        let token = Token::from_bytes([self.next.get().wrapping_mul(7); 16]);
        self.next.set(self.next.get() + 1);
        let new = NewClaim { site: site.id, name: name.parse().unwrap(), token, at: at(10) };
        pollster::block_on(self.control.claim_domain(new))
    }

    fn publish(&self, claim: &control::Claim) {
        let challenge = Challenge::of(claim);
        self.dns_fake().publish(&challenge.host, challenge.value);
    }

    fn dns_fake(&self) -> &Fake {
        self.dns.resolver()
    }

    fn prove(
        &self,
        site: &Site,
        name: &str,
        n: i64,
    ) -> Result<control::Proven, Error<Wired, authority_dns::Unreachable>> {
        let name: DomainName = name.parse().unwrap();
        pollster::block_on(self.dns.prove(&self.control, (site.id, name), at(n)))
    }

    fn served(&self, name: &str) -> Option<Site> {
        pollster::block_on(self.control.domained(&name.parse().unwrap())).unwrap()
    }
}

fn refusal<R>(result: Result<R, Error<Wired, authority_dns::Unreachable>>) -> Refusal {
    match result {
        Err(Error::Control(error)) => error.refusal().cloned().unwrap_or_else(|| panic!("not a refusal: {error}")),
        Err(other) => panic!("not a refusal: {other}"),
        Ok(_) => panic!("proved"),
    }
}

#[test]
fn a_proof_binds_the_name_to_the_site_that_claimed_it() {
    let world = World::new();
    let (tea, jam) = (world.site("tea"), world.site("jam"));
    let mine = world.claim(&tea, "tea.example.com").unwrap();
    let theirs = world.claim(&jam, "tea.example.com").unwrap();
    assert_ne!(mine.token, theirs.token);
    assert!(world.served("tea.example.com").is_none(), "a claim serves nothing");

    // the owner of the name publishes the record for the first site
    world.publish(&mine);
    assert_eq!(
        refusal(world.prove(&jam, "tea.example.com", 20)),
        Refusal::Unproven { name: mine.name.clone(), records: 1 }
    );
    assert!(world.served("tea.example.com").is_none());
    let proven = world.prove(&tea, "tea.example.com", 21).unwrap();
    assert_eq!((proven.site, proven.at), (tea.id, at(21)));
    assert_eq!(world.served("tea.example.com").unwrap().handle, tea.handle);
}

#[test]
fn a_second_site_cannot_take_a_proven_name() {
    let world = World::new();
    let (tea, jam) = (world.site("tea"), world.site("jam"));
    let mine = world.claim(&tea, "tea.example.com").unwrap();
    let late = world.claim(&jam, "tea.example.com").unwrap();
    world.publish(&mine);
    world.prove(&tea, "tea.example.com", 20).unwrap();
    // even if the late claim's record is published too, the name is the first site's
    world.publish(&late);
    let name: DomainName = "tea.example.com".parse().unwrap();
    assert_eq!(refusal(world.prove(&jam, "tea.example.com", 21)), Refusal::DomainTaken { name: name.clone() });
    let again = NewClaim { site: jam.id, name: name.clone(), token: Token::from_bytes([9; 16]), at: at(22) };
    assert_eq!(
        pollster::block_on(world.control.claim_domain(again)).unwrap_err().refusal(),
        Some(&Refusal::DomainTaken { name })
    );
    assert_eq!(world.served("tea.example.com").unwrap().id, tea.id);
    // the site that has it asks again and is told it has it
    assert!(matches!(world.claim(&tea, "tea.example.com").unwrap_err().refusal(), Some(Refusal::Exists { .. })));
}

#[test]
fn a_lookup_that_got_no_answer_proves_nothing_and_says_so() {
    let world = World::new();
    let tea = world.site("tea");
    let claim = world.claim(&tea, "tea.example.com").unwrap();
    world.publish(&claim);
    world.dns_fake().fail(true);
    let failed = world.prove(&tea, "tea.example.com", 20).unwrap_err();
    assert!(matches!(failed, Error::Lookup(_)));
    assert_eq!(failed.code(), Some("domain.lookup.failed"));
    assert!(world.served("tea.example.com").is_none());
    world.dns_fake().fail(false);
    world.prove(&tea, "tea.example.com", 21).unwrap();
    // no record, then another site's record: both are the same refusal with the count
    let other = world.claim(&tea, "www.example.com").unwrap();
    assert_eq!(
        refusal(world.prove(&tea, "www.example.com", 22)),
        Refusal::Unproven { name: other.name.clone(), records: 0 }
    );
    world.dns_fake().publish(&Host::of(&other.name), "v=spf1 -all");
    assert_eq!(refusal(world.prove(&tea, "www.example.com", 23)), Refusal::Unproven { name: other.name, records: 1 });
    // proving a name nobody claimed is a refusal of its own
    let nobody: DomainName = "none.example.com".parse().unwrap();
    assert_eq!(refusal(world.prove(&tea, "none.example.com", 24)), Refusal::NoClaim { site: tea.id, name: nobody });
}

#[test]
fn a_removed_domain_stops_resolving_and_is_free() {
    let world = World::new();
    let (tea, jam) = (world.site("tea"), world.site("jam"));
    let claim = world.claim(&tea, "tea.example.com").unwrap();
    world.publish(&claim);
    world.prove(&tea, "tea.example.com", 20).unwrap();
    let name: DomainName = "tea.example.com".parse().unwrap();
    // another site cannot remove it
    let stranger = pollster::block_on(world.control.remove_domain(jam.id, &name, at(30))).unwrap_err();
    assert_eq!(stranger.refusal(), Some(&Refusal::NoDomain { site: jam.id, name: name.clone() }));
    assert!(world.served("tea.example.com").is_some());
    pollster::block_on(world.control.remove_domain(tea.id, &name, at(31))).unwrap();
    assert!(world.served("tea.example.com").is_none());
    let listed = pollster::block_on(world.control.domains(tea.id)).unwrap();
    assert!(listed.proven.is_empty() && listed.pending.is_empty());
    // the name is free: the other site claims and proves it with a record of its own
    let theirs = world.claim(&jam, "tea.example.com").unwrap();
    world.publish(&theirs);
    world.prove(&jam, "tea.example.com", 40).unwrap();
    assert_eq!(world.served("tea.example.com").unwrap().id, jam.id);
}

#[test]
fn a_removed_site_releases_its_domains_and_a_renamed_one_keeps_them() {
    let world = World::new();
    let (tea, jam) = (world.site("tea"), world.site("jam"));
    let claim = world.claim(&tea, "tea.example.com").unwrap();
    world.publish(&claim);
    world.prove(&tea, "tea.example.com", 20).unwrap();
    world.claim(&tea, "pending.example.com").unwrap();
    // the domain belongs to the site, so a new handle serves it too
    pollster::block_on(world.control.rename_site(tea.id, "leaf".parse().unwrap(), at(25))).unwrap();
    assert_eq!(world.served("tea.example.com").unwrap().handle.as_str(), "leaf");
    // removing the site frees both the proven name and the pending claim, in the same step as its rows
    pollster::block_on(world.control.remove_site(tea.id, at(30))).unwrap();
    assert!(world.served("tea.example.com").is_none());
    let theirs = world.claim(&jam, "tea.example.com").unwrap();
    world.publish(&theirs);
    world.prove(&jam, "tea.example.com", 40).unwrap();
    assert_eq!(world.served("tea.example.com").unwrap().id, jam.id);
    world.claim(&jam, "pending.example.com").unwrap();
}

#[test]
fn a_name_the_operator_serves_or_too_many_names_are_refused() {
    let world = World::new();
    let tea = world.site("tea");
    for ours in ["uses.ink", "domains.uses.ink", "tea.uses.ink", "main--tea.uses.ink"] {
        let name: DomainName = ours.parse().unwrap();
        assert_eq!(world.claim(&tea, ours).unwrap_err().refusal(), Some(&Refusal::DomainOurs { name }));
    }
    world.claim(&tea, "notuses.ink").unwrap();
    let limited = world.control.with_policy(control::Policy::DEFAULT.domained(2));
    let world = World { control: limited, ..world };
    world.claim(&tea, "a.example.com").unwrap();
    // asking again for a name it already asked is not a new name
    world.claim(&tea, "a.example.com").unwrap();
    let full = world.claim(&tea, "b.example.com").unwrap_err();
    assert_eq!(full.refusal(), Some(&Refusal::DomainRoom { site: tea.id, limit: 2 }));
}

#[test]
fn a_claim_made_again_has_a_new_token_that_the_old_record_does_not_prove() {
    let world = World::new();
    let tea = world.site("tea");
    let first = world.claim(&tea, "tea.example.com").unwrap();
    world.publish(&first);
    let second = world.claim(&tea, "tea.example.com").unwrap();
    assert_ne!(first.token, second.token);
    let listed = pollster::block_on(world.control.domains(tea.id)).unwrap();
    assert_eq!(listed.pending, vec![second.clone()]);
    assert!(matches!(refusal(world.prove(&tea, "tea.example.com", 20)), Refusal::Unproven { .. }));
    world.publish(&second);
    world.prove(&tea, "tea.example.com", 21).unwrap();
}
