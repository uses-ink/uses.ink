//! Repository claims in the control plane alone: the claim is a row with a challenge and a sealed
//! secret, a proof binds one repository to one site, and every change is an event of the site.

use authority_forge::{Challenge, Proven, Repo};
use build::Source;
use control::table;
use control::{
    AccountKind, Binding, Handle, Id, NewAccount, NewRepo, NewSite, Refusal, Releasing, Rotating, Site, SiteKind,
};
use sea_query::ExprTrait as _;
use secret::Plain;

use crate::world::{World, at};

fn repo(text: &str) -> Repo {
    text.parse().unwrap()
}

impl World {
    /// A site of a new account that builds the git repository at `url`.
    fn git_site(&self, handle: &str, url: &str) -> Site {
        let account: Id<AccountKind> = self.id();
        self.run(self.control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
        let source = Source::Git { url: url.parse().unwrap(), revision: None };
        let new = NewSite {
            id: self.id::<SiteKind>(),
            account,
            handle: handle.parse::<Handle>().unwrap(),
            source,
            at: at(1),
        };
        self.run(self.control.add_site(new)).unwrap()
    }

    fn asking(&self, site: &Site, name: &str, n: i64) -> NewRepo {
        NewRepo {
            site: site.id,
            repo: repo(name),
            challenge: Challenge::from_bytes([n.to_le_bytes()[0]; 16]),
            secret: Plain::from(format!("secret-{n}").into_bytes()),
            credential: self.credential(),
            at: at(n),
        }
    }
}

fn proof(name: &str) -> Proven {
    Proven::of(repo(name))
}

#[test]
fn a_claim_keeps_its_challenge_and_seals_its_secret() {
    let world = World::new();
    let site = world.git_site("tea", "https://codeberg.org/Octo/Docs.git");
    let asked = world.asking(&site, "codeberg.org/octo/docs", 10);
    let claim = world.run(world.control.claim_repo(asked)).unwrap();
    assert!(!claim.bound());
    assert_eq!(claim.repo.to_string(), "codeberg.org/octo/docs");
    assert_eq!(world.events("repo.claimed"), 1);
    let listed = world.run(world.control.repos(site.id)).unwrap();
    assert_eq!(listed, std::slice::from_ref(&claim));
    let opened = world.run(world.control.hook_secret(&claim)).unwrap();
    assert_eq!(opened.expose(), b"secret-10");
    // the stored column is the sealed bytes, never the secret
    let rows = world.rows(sea_query::Query::select().column(table::Credential::Sealed).from(table::Credential::Table));
    let sealed: Vec<u8> = rows[0].get(table::Credential::Sealed).unwrap();
    assert!(!sealed.windows(9).any(|window| window == b"secret-10"));
    // asking again replaces the challenge and the secret, and leaves one of each
    let again = world.run(world.control.claim_repo(world.asking(&site, "codeberg.org/octo/docs", 11))).unwrap();
    assert_ne!(again.challenge, claim.challenge);
    assert_eq!(world.count(table::Repo::Table), 1);
    assert_eq!(
        world.counted(table::Credential::Table, sea_query::Expr::col(table::Credential::Kind).eq("webhook-secret")),
        1
    );
    assert!(world.run(world.control.hook_secret(&claim)).is_err(), "the earlier secret is gone");
}

#[test]
fn only_the_sites_own_repository_can_be_claimed() {
    let world = World::new();
    let git = world.git_site("tea", "https://codeberg.org/octo/docs");
    let other = world.asking(&git, "codeberg.org/octo/other", 10);
    let refused = world.run(world.control.claim_repo(other)).unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refusal::RepoSource { site: git.id, repo: repo("codeberg.org/octo/other") }));
    let folder = world.site("journal");
    let refused = world.run(world.control.claim_repo(world.asking(&folder, "codeberg.org/octo/docs", 11))).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::RepoSource { .. })));
    assert_eq!(world.count(table::Repo::Table), 0);
}

#[test]
fn a_proof_binds_the_repository_to_the_site_that_claimed_it() {
    let world = World::new();
    let first = world.git_site("tea", "https://codeberg.org/octo/docs");
    let second = world.git_site("tisane", "https://codeberg.org/octo/docs");
    world.run(world.control.claim_repo(world.asking(&first, "codeberg.org/octo/docs", 10))).unwrap();
    world.run(world.control.claim_repo(world.asking(&second, "codeberg.org/octo/docs", 11))).unwrap();
    assert_eq!(world.count(table::Repo::Table), 2, "any number of sites may ask before one proves");
    // a proof for a repository a site did not claim binds nothing
    let none = Binding { site: first.id, proof: proof("codeberg.org/octo/else"), at: at(20) };
    let refused = world.run(world.control.bind_repo(none)).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::NoRepo { .. })));
    let bound = world
        .run(world.control.bind_repo(Binding { site: second.id, proof: proof("codeberg.org/octo/docs"), at: at(21) }))
        .unwrap();
    assert_eq!(bound.proven, Some(at(21)));
    assert_eq!(world.events("repo.proven"), 1);
    let holder = world.run(world.control.bound(&repo("codeberg.org/octo/docs"))).unwrap().unwrap();
    assert_eq!(holder.site, second.id);
    // the other site's claim and its secret are gone, and it cannot claim or prove again
    assert!(world.run(world.control.repos(first.id)).unwrap().is_empty());
    assert_eq!(world.count(table::Credential::Table), 1);
    let taken = world.run(world.control.claim_repo(world.asking(&first, "codeberg.org/octo/docs", 30))).unwrap_err();
    assert_eq!(taken.refusal(), Some(&Refusal::RepoTaken { repo: repo("codeberg.org/octo/docs") }));
    // proving again answers the row it has and writes nothing
    let again = world
        .run(world.control.bind_repo(Binding { site: second.id, proof: proof("codeberg.org/octo/docs"), at: at(40) }))
        .unwrap();
    assert_eq!(again, bound);
    assert_eq!(world.events("repo.proven"), 1);
    let mine = world.run(world.control.claim_repo(world.asking(&second, "codeberg.org/octo/docs", 41))).unwrap_err();
    assert!(matches!(mine.refusal(), Some(Refusal::Exists { .. })));
}

#[test]
fn the_database_keeps_one_proof_of_a_repository() {
    use control::Ports as _;
    use db::Db as _;
    let world = World::new();
    let first = world.git_site("tea", "https://codeberg.org/octo/docs");
    let second = world.git_site("tisane", "https://codeberg.org/octo/docs");
    world.run(world.control.claim_repo(world.asking(&first, "codeberg.org/octo/docs", 10))).unwrap();
    world.run(world.control.claim_repo(world.asking(&second, "codeberg.org/octo/docs", 11))).unwrap();
    // both rows marked proved, as two concurrent proofs would: the unique key refuses the second
    let mark = |site: Id<SiteKind>| -> db::Statement {
        sea_query::Query::update()
            .table(table::Repo::Table)
            .value(table::Repo::Owned, 1_i64)
            .and_where(sea_query::Expr::col(table::Repo::SiteId).eq(site.to_string()))
            .to_owned()
            .into()
    };
    world.run(world.control.ports().db().batch(vec![mark(first.id)])).unwrap();
    let error = world.run(world.control.ports().db().batch(vec![mark(second.id)])).unwrap_err();
    assert_eq!(error.constraint(), Some(db::Constraint::Unique));
}

#[test]
fn a_claim_is_released_with_its_secret() {
    let world = World::new();
    let site = world.git_site("tea", "https://codeberg.org/octo/docs");
    world.run(world.control.claim_repo(world.asking(&site, "codeberg.org/octo/docs", 10))).unwrap();
    let released = Releasing { site: site.id, repo: repo("codeberg.org/octo/docs"), at: at(12) };
    world.run(world.control.release_repo(released)).unwrap();
    assert_eq!(world.count(table::Credential::Table), 0, "the secret goes with the claim");
    assert_eq!(world.count(table::Repo::Table), 0);
    assert_eq!(world.events("repo.released"), 1);
}

#[test]
fn a_secret_is_rotated_and_the_old_one_is_gone() {
    let world = World::new();
    let site = world.git_site("tea", "https://codeberg.org/octo/docs");
    let claim = world.run(world.control.claim_repo(world.asking(&site, "codeberg.org/octo/docs", 10))).unwrap();
    let rotating = Rotating {
        site: site.id,
        repo: claim.repo.clone(),
        secret: Plain::from("rotated"),
        credential: world.credential(),
        at: at(20),
    };
    let rotated = world.run(world.control.rotate_repo(rotating)).unwrap();
    assert_ne!(rotated.secret, claim.secret);
    assert_eq!(world.run(world.control.hook_secret(&rotated)).unwrap().expose(), b"rotated");
    assert!(world.run(world.control.hook_secret(&claim)).is_err());
    assert_eq!(world.count(table::Credential::Table), 1);
    assert_eq!(world.events("repo.rotated"), 1);
    let none = Rotating {
        site: site.id,
        repo: repo("codeberg.org/octo/else"),
        secret: Plain::from("x"),
        credential: world.credential(),
        at: at(21),
    };
    let refused = world.run(world.control.rotate_repo(none)).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::NoRepo { .. })));
}

#[test]
fn releasing_what_was_never_claimed_is_refused_and_removing_the_site_takes_its_claims() {
    let world = World::new();
    let site = world.git_site("tea", "https://codeberg.org/octo/docs");
    let missing = Releasing { site: site.id, repo: repo("codeberg.org/octo/docs"), at: at(5) };
    let refused = world.run(world.control.release_repo(missing)).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::NoRepo { .. })));
    world.run(world.control.claim_repo(world.asking(&site, "codeberg.org/octo/docs", 10))).unwrap();
    world
        .run(world.control.bind_repo(Binding { site: site.id, proof: proof("codeberg.org/octo/docs"), at: at(11) }))
        .unwrap();
    world.run(world.control.remove_site(site.id, at(30))).unwrap();
    assert_eq!(world.count(table::Repo::Table), 0);
    assert!(world.run(world.control.bound(&repo("codeberg.org/octo/docs"))).unwrap().is_none());
}
