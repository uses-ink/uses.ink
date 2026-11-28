//! Organizations: the role table as a property, the last owner, and a deploy key that outlives
//! the member who approved it.

use control::keys::{
    Approval, Keys, Kind, Label, NewEnrolment, Refused as KeyRefused, Right, Rights, Scope, Settings, UserCode,
};
use control::{AccountKind, Handle, Id, NewAccount, NewOrg, NewSite, Ports, Refusal, Role, Seat};
use sig::{KeyType, Seed, Signer};

use crate::world::{World, at};

impl World {
    /// A person: an account with no site.
    fn person(&self) -> Id<AccountKind> {
        let id = self.id::<AccountKind>();
        self.run(self.control.add_account(NewAccount { id, at: at(0) })).unwrap();
        id
    }

    /// An organization made by a new person, who is its first owner.
    fn org(&self, name: &str) -> (Id<AccountKind>, Id<AccountKind>) {
        let owner = self.person();
        let id = self.id::<AccountKind>();
        self.run(self.control.add_org(NewOrg { id, owner, name: name.parse().unwrap(), at: at(1) })).unwrap();
        (id, owner)
    }

    /// A new person added to `org` with `role` by `by`.
    fn join(&self, org: Id<AccountKind>, by: Id<AccountKind>, role: Role) -> Id<AccountKind> {
        let whom = self.person();
        self.run(self.control.add_member(Seat { org, by, whom }, role, at(2))).unwrap();
        whom
    }

    fn refusal<T: std::fmt::Debug>(result: Result<T, control::Error<crate::world::Wired>>) -> Refusal {
        result.unwrap_err().refusal().cloned().unwrap()
    }
}

fn denied(refusal: &Refusal) -> bool {
    matches!(refusal, Refusal::Role { .. })
}

#[test]
fn an_organization_has_its_maker_as_the_only_owner() {
    let world = World::new();
    let (org, owner) = world.org("Garden club");
    let members = world.run(world.control.members(org)).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!((members[0].account, members[0].role), (owner, Role::Owner));
    assert_eq!(world.run(world.control.org(org)).unwrap().unwrap().name, "Garden club");
    let grants = world.run(world.control.grants(owner)).unwrap();
    assert_eq!(grants.list().len(), 1);
    assert_eq!(grants.list()[0].scope, Scope::Account(org));
    assert_eq!(grants.list()[0].rights, Rights::EVERY);
    // a person who is nobody's member has no grant from an org
    assert!(world.run(world.control.grants(world.person())).unwrap().list().is_empty());
    let stranger = world.id::<AccountKind>();
    assert_eq!(
        World::refusal(world.run(world.control.add_org(NewOrg {
            id: world.id(),
            owner: stranger,
            name: "x".parse().unwrap(),
            at: at(3)
        }))),
        Refusal::NoAccount { account: stranger }
    );
}

#[test]
fn who_may_change_whom_follows_the_role_table_for_every_pair() {
    for actor in Role::ALL {
        // a world per actor keeps the ids a test draws within the world's counter
        let world = World::new();
        for target in Role::ALL {
            for becomes in Role::ALL {
                let (org, owner) = world.org("o");
                // a second owner, so the last-owner rule never decides this case
                world.join(org, owner, Role::Owner);
                let by = if actor == Role::Owner { owner } else { world.join(org, owner, actor) };
                let whom = world.join(org, owner, target);
                let seat = Seat { org, by, whom };
                let expect = actor.manages(target) && actor.manages(becomes);
                let changed = world.run(world.control.set_role(seat, becomes));
                assert_eq!(changed.is_ok(), expect, "{actor} sets {target} to {becomes}");
                if !expect && by != whom {
                    assert!(denied(&World::refusal(changed)), "{actor} sets {target} to {becomes}");
                }
                // removal: needs to manage the role the member has now
                let now = world.run(world.control.role(org, whom)).unwrap().unwrap();
                let removed = world.run(world.control.remove_member(seat));
                assert_eq!(removed.is_ok(), actor.manages(now), "{actor} removes {now}");
                if removed.is_ok() {
                    assert_eq!(world.run(world.control.role(org, whom)).unwrap(), None);
                }
            }
        }
    }
}

#[test]
fn a_writer_cannot_manage_members_and_an_admin_cannot_make_an_owner() {
    let world = World::new();
    let (org, owner) = world.org("o");
    let writer = world.join(org, owner, Role::Writer);
    let admin = world.join(org, owner, Role::Admin);
    let newcomer = world.person();
    let by_writer = world.run(world.control.add_member(Seat { org, by: writer, whom: newcomer }, Role::Reader, at(5)));
    assert_eq!(World::refusal(by_writer), Refusal::Role { org, role: Role::Writer, needs: Right::Members });
    let by_admin = world.run(world.control.add_member(Seat { org, by: admin, whom: newcomer }, Role::Owner, at(5)));
    assert_eq!(World::refusal(by_admin), Refusal::Role { org, role: Role::Admin, needs: Right::Members });
    assert!(world.run(world.control.add_member(Seat { org, by: admin, whom: newcomer }, Role::Writer, at(5))).is_ok());
    // an admin cannot raise themselves, and cannot remove an owner
    let raised = world.run(world.control.set_role(Seat { org, by: admin, whom: admin }, Role::Owner));
    assert!(denied(&World::refusal(raised)));
    let removed = world.run(world.control.remove_member(Seat { org, by: admin, whom: owner }));
    assert!(denied(&World::refusal(removed)));
    // a stranger to the organization has no seat to act from
    let outsider = world.person();
    let by_outsider =
        world.run(world.control.add_member(Seat { org, by: outsider, whom: world.person() }, Role::Reader, at(5)));
    assert_eq!(World::refusal(by_outsider), Refusal::NotMember { org, account: outsider });
    // a member is added once
    let twice = world.run(world.control.add_member(Seat { org, by: owner, whom: writer }, Role::Reader, at(5)));
    assert_eq!(World::refusal(twice), Refusal::Member { org, account: writer });
}

#[test]
fn the_last_owner_cannot_be_removed_or_demoted_and_a_second_owner_changes_that() {
    let world = World::new();
    let (org, owner) = world.org("o");
    let alone = Seat { org, by: owner, whom: owner };
    assert_eq!(World::refusal(world.run(world.control.remove_member(alone))), Refusal::LastOwner { org });
    assert_eq!(World::refusal(world.run(world.control.set_role(alone, Role::Admin))), Refusal::LastOwner { org });
    assert_eq!(world.run(world.control.role(org, owner)).unwrap(), Some(Role::Owner));
    // keeping the role is no demotion
    assert!(world.run(world.control.set_role(alone, Role::Owner)).is_ok());
    let second = world.join(org, owner, Role::Owner);
    // either may go, but not both
    assert!(world.run(world.control.set_role(Seat { org, by: second, whom: owner }, Role::Writer)).is_ok());
    let last = Seat { org, by: second, whom: second };
    assert_eq!(World::refusal(world.run(world.control.remove_member(last))), Refusal::LastOwner { org });
    assert_eq!(
        World::refusal(world.run(world.control.remove_member(Seat { org, by: owner, whom: second }))),
        Refusal::Role { org, role: Role::Writer, needs: Right::Members }
    );
    // an unknown member is not a last owner
    let nobody = world.person();
    assert_eq!(
        World::refusal(world.run(world.control.remove_member(Seat { org, by: second, whom: nobody }))),
        Refusal::NotMember { org, account: nobody }
    );
}

#[test]
fn an_org_deploy_key_outlives_the_member_who_approved_it_and_is_listed_as_orphaned() {
    let world = World::new();
    let (org, owner) = world.org("o");
    let writer = world.join(org, owner, Role::Writer);
    let reader = world.join(org, owner, Role::Reader);
    let site = world
        .run(world.control.add_site(NewSite {
            id: world.id(),
            account: org,
            handle: "club".parse::<Handle>().unwrap(),
            source: build::Source::Folder("/srv/club".into()),
            at: at(3),
        }))
        .unwrap();
    let keys = Keys::new(world.control.ports().db().clone(), Settings::default());
    let enrol = |n: u8| NewEnrolment {
        id: world.id(),
        code: UserCode::drawn([n; 16]),
        key_type: KeyType::Ed25519V1,
        public: Signer::from_seed(&Seed::new([n; 32])).public(),
        label: Label::parse("ci").unwrap(),
        purpose: Kind::Deploy,
        site: Some(site.id),
        rights: Rights::NONE,
        lifetime: None,
        at: at(10),
    };
    let approval = |new: &NewEnrolment, by| Approval {
        code: new.code.clone(),
        fingerprint: new.public.id(new.key_type).fingerprint(),
        by,
        within: Rights::EVERY,
    };
    let (mine, theirs) = (enrol(1), enrol(2));
    world.run(keys.enrol(mine.clone())).unwrap();
    world.run(keys.enrol(theirs.clone())).unwrap();
    // a reader gives no one a way to deploy; a stranger cannot approve at all
    let by_reader = world.run(world.control.approve_for(org, approval(&mine, reader), at(20)));
    assert_eq!(World::refusal(by_reader), Refusal::Role { org, role: Role::Reader, needs: Right::Deploy });
    let outsider = world.person();
    let by_outsider = world.run(world.control.approve_for(org, approval(&mine, outsider), at(20)));
    assert_eq!(World::refusal(by_outsider), Refusal::NotMember { org, account: outsider });
    let key = world.run(world.control.approve_for(org, approval(&mine, writer), at(21))).unwrap();
    assert_eq!((key.account, key.approved_by, key.grant.scope), (org, writer, Scope::Site(site.id)));
    assert_eq!(key.grant.rights, Rights::DEPLOY);
    let listed = world.run(world.control.org_keys(org)).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].creator_removed);
    // the writer leaves; the key stays, live and unrevoked, and says who is gone
    world.run(world.control.remove_member(Seat { org, by: writer, whom: writer })).unwrap();
    let listed = world.run(world.control.org_keys(org)).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].creator_removed && listed[0].key.revoked.is_none());
    assert_eq!(world.run(keys.key(key.id)).unwrap().unwrap().account, org);
    // a person's own device key cannot be given to an organization
    let device = NewEnrolment { purpose: Kind::Device, site: None, ..enrol(3) };
    world.run(keys.enrol(device.clone())).unwrap();
    let wrong = world.run(keys.approve_as(approval(&device, owner), org, at(22)));
    assert_eq!(wrong.unwrap_err().refusal(), Some(&KeyRefused::Site));
    let _ = theirs;
}

#[test]
fn a_person_lists_their_organizations_by_name_with_their_role_and_its_size() {
    let world = World::new();
    let (zoo, owner) = world.org("Zoo");
    let (alpha, other) = world.org("Alpha");
    world.join(alpha, other, Role::Reader);
    world.run(world.control.add_member(Seat { org: alpha, by: other, whom: owner }, Role::Writer, at(3))).unwrap();
    let standing = world.run(world.control.orgs_of(owner)).unwrap();
    let seen: Vec<(String, Role, u64)> = standing.iter().map(|s| (s.org.name.clone(), s.role, s.members)).collect();
    assert_eq!(seen, [("Alpha".to_owned(), Role::Writer, 3), ("Zoo".to_owned(), Role::Owner, 1)]);
    assert_eq!((standing[0].org.id, standing[1].org.id), (alpha, zoo));
    assert!(world.run(world.control.orgs_of(world.person())).unwrap().is_empty());
}

#[test]
fn a_roster_says_what_a_member_may_give_a_key_and_refuses_the_rest() {
    let world = World::new();
    let (org, owner) = world.org("Garden club");
    let reader = world.join(org, owner, Role::Reader);
    let writer = world.join(org, owner, Role::Writer);
    let roster = control::Roster::new(world.control.ports().db());
    let allowed = |person| world.run(roster.allowed(org, person));
    assert_eq!(allowed(owner).unwrap(), Rights::EVERY);
    assert_eq!(allowed(writer).unwrap(), Role::Writer.rights());
    assert!(matches!(allowed(reader), Err(control::Seated::Role { role: Role::Reader, needs: Right::Deploy, .. })));
    let stranger = world.person();
    assert!(matches!(allowed(stranger), Err(control::Seated::NotMember { .. })));
}

#[test]
fn an_organization_name_is_one_line() {
    assert!("Garden club".parse::<control::OrgName>().is_ok());
    assert!(matches!("".parse::<control::OrgName>(), Err(control::OrgNameError::Empty)));
}
