//! Organizations: making one, its members and their roles, and acting as one (decision 0095).

use authority_dns::Resolver;
use authority_forge::Transport;
use clock::{Clock, Draw};
use control::keys::{Kind, Scope};
use control::{AccountKind, Id, NewOrg, Ports, Refusal, Role, Seat, Standing};

use crate::Api;
use crate::auth::credential::{Credentials, Principal, Via};
use crate::body::{AddMember, AddOrg, ChangeRole, Unreadable};
use crate::fault::Fault;
use crate::reply::{Reply, Status};
use crate::route::Call;
use crate::view::{KeyView, Listed, MemberView, OrgMembers, OrgView, Unmembered};

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw, R: Resolver, T: Transport> Api<P, C, K, D, R, T> {
    /// The same call made as the organization `org`: the account it acts for, the site's owner it
    /// is checked against and the key's reach change together. A key of a person reaches the
    /// organizations they are a member of with what both the key and their role allow; a deploy key
    /// reaches only the organization it belongs to; anyone else finds no organization.
    pub(crate) async fn acting(&self, principal: Principal, org: Id<AccountKind>) -> Result<Principal, Fault<P>> {
        if principal.account == org {
            return Ok(principal);
        }
        let person = matches!(principal.via, Via::Operator { .. } | Via::Key { kind: Kind::Device, .. });
        let orgs = self.control.grants(principal.person).await?;
        if !person || !orgs.list().iter().any(|grant| grant.scope == Scope::Account(org)) {
            return Err(Refusal::NoOrg { org }.into());
        }
        let effective = principal.effective.clone().joined(principal.person, &orgs);
        Ok(Principal { account: org, effective, ..principal })
    }

    /// The calls about organizations and their members.
    pub(crate) async fn members(&self, principal: &Principal, call: Call, body: &[u8]) -> Result<Reply, Fault<P>> {
        match call {
            Call::OrgAdd => self.org_add(principal, body).await,
            Call::OrgShow(_) => self.org_show(principal).await,
            Call::MemberAdd(_) => self.member_add(principal, body).await,
            Call::MemberRole { account, .. } => self.member_role(principal, account, body).await,
            Call::MemberOut { account, .. } => self.member_out(principal, account).await,
            _ => self.orgs(principal).await,
        }
    }

    /// `GET /v1/orgs`: the organizations the caller is a member of.
    pub(crate) async fn orgs(&self, principal: &Principal) -> Result<Reply, Fault<P>> {
        let standing = self.control.orgs_of(principal.person).await?;
        Self::json(Status::Ok, &Listed { items: standing.iter().map(OrgView::from).collect() })
    }

    /// `POST /v1/orgs`: makes an organization, with the caller as its only owner.
    pub(crate) async fn org_add(&self, principal: &Principal, body: &[u8]) -> Result<Reply, Fault<P>> {
        let new: AddOrg = Unreadable::read(body).map_err(Fault::Body)?;
        let new = NewOrg {
            id: Id::from_bytes(self.drawn()?),
            owner: principal.person,
            name: new.name.0,
            at: self.clock.now(),
        };
        let org = self.control.add_org(new).await?;
        let standing = Standing { org, role: Role::Owner, members: 1 };
        Self::json(Status::Created, &OrgView::from(&standing))
    }

    /// `GET /v1/orgs/{org}`: the organization and its members. A person who is no member finds no
    /// organization.
    pub(crate) async fn org_show(&self, principal: &Principal) -> Result<Reply, Fault<P>> {
        let org = principal.account;
        let members = self.control.members(org).await?;
        let found = self.control.org(org).await?.ok_or(Refusal::NoOrg { org })?;
        let role = self.control.role(org, principal.person).await?.ok_or(Refusal::NoOrg { org })?;
        let count = u64::try_from(members.len()).unwrap_or(u64::MAX);
        let view = OrgView::from(&Standing { org: found, role, members: count });
        Self::json(Status::Ok, &OrgMembers { org: view, members: members.iter().map(MemberView::from).collect() })
    }

    /// `POST /v1/orgs/{org}/members`: adds a person, when the caller's role manages the one given.
    pub(crate) async fn member_add(&self, principal: &Principal, body: &[u8]) -> Result<Reply, Fault<P>> {
        let new: AddMember = Unreadable::read(body).map_err(Fault::Body)?;
        let seat = Seat { org: principal.account, by: principal.person, whom: new.account.0 };
        let member = self.control.add_member(seat, new.role.0, self.clock.now()).await?;
        Self::json(Status::Created, &MemberView::from(&member))
    }

    /// `PUT /v1/orgs/{org}/members/{account}`: gives a member another role.
    pub(crate) async fn member_role(
        &self,
        principal: &Principal,
        whom: Id<AccountKind>,
        body: &[u8],
    ) -> Result<Reply, Fault<P>> {
        let new: ChangeRole = Unreadable::read(body).map_err(Fault::Body)?;
        let seat = Seat { org: principal.account, by: principal.person, whom };
        let member = self.control.set_role(seat, new.role.0).await?;
        Self::json(Status::Ok, &MemberView::from(&member))
    }

    /// `DELETE /v1/orgs/{org}/members/{account}`: removes a member. The keys they approved stay.
    pub(crate) async fn member_out(&self, principal: &Principal, whom: Id<AccountKind>) -> Result<Reply, Fault<P>> {
        let seat = Seat { org: principal.account, by: principal.person, whom };
        self.control.remove_member(seat).await?;
        Self::json(Status::Ok, &Unmembered { org: principal.account, account: whom })
    }

    /// `GET /v1/orgs/{org}/keys`: the organization's keys, each with whether the member who
    /// approved it is still one.
    pub(crate) async fn org_keys(&self, principal: &Principal) -> Result<Reply, Fault<P>> {
        let keys = self.control.org_keys(principal.account).await?;
        let items = keys
            .iter()
            .map(|listed| KeyView { creator_removed: Some(listed.creator_removed), ..KeyView::from(&listed.key) })
            .collect();
        Self::json(Status::Ok, &Listed { items })
    }
}
