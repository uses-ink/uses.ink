//! Organizations (decision 0095): making one, its members and their roles, and the keys that
//! outlive the member who approved them. Who may manage whom is [`Role::manages`]; what must
//! hold at once is in a statement's `WHERE`.

use db::{Constraint, Db};
use jiff::Timestamp;
use sea_query::{Expr, ExprTrait, Order, Query, SelectStatement};

use crate::id::{Account as AccountKind, Id};
use crate::keys::{Approval, Grant, Grants, Key, Keys, Right, Scope};
use crate::org::{Member, NewOrg, Org, OrgKey, Role, Standing};
use crate::{Control, Error, Ports, Refusal, table};

/// A change to a member: who asks, whose seat, and what it becomes (none removes it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    /// The organization.
    pub org: Id<AccountKind>,
    /// The member who asks.
    pub by: Id<AccountKind>,
    /// The member it is about.
    pub whom: Id<AccountKind>,
}

impl<P: Ports> Control<P> {
    /// Makes an organization with `new.owner` as its only owner. The organization is an account
    /// of its own, so it owns sites and keys that no member's leaving takes away.
    pub async fn add_org(&self, new: NewOrg) -> Result<Org, Error<P>> {
        let account = Query::insert()
            .into_table(table::Account::Table)
            .columns([table::Account::Id, table::Account::CreatedAt])
            .values([Expr::val(new.id.to_string()), Expr::val(new.at.as_millisecond())])?
            .to_owned();
        let org = Query::insert()
            .into_table(table::Org::Table)
            .columns(Org::COLUMNS)
            .values([Expr::val(new.id.to_string()), Expr::val(new.name.as_str()), Expr::val(new.at.as_millisecond())])?
            .to_owned();
        let owner = Member { org: new.id, account: new.owner, role: Role::Owner, since: new.at };
        match self.ports.db().batch(vec![account.into(), org.into(), owner.insert(None)?]).await {
            Ok(_) => Ok(Org { id: new.id, name: new.name.to_string(), created: new.at }),
            Err(error) if error.constraint() == Some(Constraint::Unique) => {
                Err(Refusal::Exists { id: new.id.to_string() }.into())
            }
            Err(error) if error.constraint() == Some(Constraint::Reference) => {
                Err(Refusal::NoAccount { account: new.owner }.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// The organization, if there is one.
    pub async fn org(&self, id: Id<AccountKind>) -> Result<Option<Org>, Error<P>> {
        let select = Query::select()
            .columns(Org::COLUMNS)
            .from(table::Org::Table)
            .and_where(Expr::col(table::Org::Id).eq(id.to_string()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Org::read(&row)).transpose()?)
    }

    /// The members of an organization, owners first and then by when they joined.
    pub async fn members(&self, org: Id<AccountKind>) -> Result<Vec<Member>, Error<P>> {
        self.org(org).await?.ok_or(Refusal::NoOrg { org })?;
        let select = Query::select()
            .columns(Member::COLUMNS)
            .from(table::Member::Table)
            .and_where(Expr::col(table::Member::OrgId).eq(org.to_string()))
            .order_by(table::Member::CreatedAt, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        let mut members = rows.iter().map(Member::read).collect::<Result<Vec<_>, _>>()?;
        members.sort_by_key(|member| member.role);
        Ok(members)
    }

    /// The role `account` has in `org`, if it is a member.
    pub async fn role(&self, org: Id<AccountKind>, account: Id<AccountKind>) -> Result<Option<Role>, Error<P>> {
        let select = Self::seat_of(org, account).columns(Member::COLUMNS).to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Member::read(&row)).transpose()?.map(|member| member.role))
    }

    /// The organizations `account` is a member of, and the grants they give it: the role's rights
    /// on the organization's account. A person's rights are the union of these and their own.
    pub async fn grants(&self, account: Id<AccountKind>) -> Result<Grants, Error<P>> {
        let select = Query::select()
            .columns(Member::COLUMNS)
            .from(table::Member::Table)
            .and_where(Expr::col(table::Member::AccountId).eq(account.to_string()))
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        let list = rows
            .iter()
            .map(|row| Member::read(row).map(|m| Grant { scope: Scope::Account(m.org), rights: m.role.rights() }))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Grants::new(list))
    }

    /// The organizations `person` is a member of, each with their role and its member count,
    /// by name.
    pub async fn orgs_of(&self, person: Id<AccountKind>) -> Result<Vec<Standing>, Error<P>> {
        let mine = Query::select()
            .columns(Member::COLUMNS)
            .from(table::Member::Table)
            .and_where(Expr::col(table::Member::AccountId).eq(person.to_string()))
            .to_owned();
        let seats = self.ports.db().rows(mine.into()).await?;
        let mut found = Vec::new();
        for seat in seats.iter().map(Member::read).collect::<Result<Vec<_>, _>>()? {
            let org = self.org(seat.org).await?.ok_or(Refusal::NoOrg { org: seat.org })?;
            let members = u64::try_from(self.members(seat.org).await?.len()).unwrap_or(u64::MAX);
            found.push(Standing { org, role: seat.role, members });
        }
        found.sort_by(|left, right| left.org.name.cmp(&right.org.name));
        Ok(found)
    }

    /// Adds `whom` to the organization with `role`, when `by` is a member who may give it. A
    /// person who is a member already is refused: changing a role is [`Control::set_role`].
    pub async fn add_member(&self, seat: Seat, role: Role, at: Timestamp) -> Result<Member, Error<P>> {
        let member = Member { org: seat.org, account: seat.whom, role, since: at };
        let guard = Self::managing(seat.org, seat.by, &[role]);
        match self.ports.db().run(member.insert(Some(guard))?).await {
            Ok(done) if done.changes == 1 => Ok(member),
            Ok(_) => Err(self.denied(seat).await?),
            Err(error) if error.constraint() == Some(Constraint::Unique) => {
                Err(Refusal::Member { org: seat.org, account: seat.whom }.into())
            }
            Err(error) if error.constraint() == Some(Constraint::Reference) => {
                Err(Refusal::NoAccount { account: seat.whom }.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Gives `whom` another role, when `by` may manage both the role they have and the one they
    /// get. An organization's last owner cannot be given a lower role: the statement that
    /// changes the row holds only while another owner remains.
    pub async fn set_role(&self, seat: Seat, role: Role) -> Result<Member, Error<P>> {
        let Some(had) = self.role(seat.org, seat.whom).await? else {
            return Err(Refusal::NotMember { org: seat.org, account: seat.whom }.into());
        };
        let update = Query::update()
            .table(table::Member::Table)
            .value(table::Member::Role, role.name())
            .and_where(Expr::col(table::Member::OrgId).eq(seat.org.to_string()))
            .and_where(Expr::col(table::Member::AccountId).eq(seat.whom.to_string()))
            .and_where(Expr::col(table::Member::Role).eq(had.name()))
            .and_where(Expr::exists(Self::managing(seat.org, seat.by, &[had, role])))
            .and_where(Self::stays(seat, role))
            .to_owned();
        if self.ports.db().run(update.into()).await?.changes != 1 {
            return Err(self.refused_change(seat, &[had, role]).await?);
        }
        let row = Self::seat_of(seat.org, seat.whom).columns(Member::COLUMNS).to_owned();
        let row =
            self.ports.db().first(row.into()).await?.ok_or(Refusal::NotMember { org: seat.org, account: seat.whom })?;
        Ok(Member::read(&row)?)
    }

    /// Removes `seat.whom` from the organization. A member may always leave; removing someone
    /// else needs a role that manages theirs. The last owner cannot be removed. Keys the person
    /// approved stay with the organization.
    pub async fn remove_member(&self, seat: Seat) -> Result<(), Error<P>> {
        let Some(had) = self.role(seat.org, seat.whom).await? else {
            return Err(Refusal::NotMember { org: seat.org, account: seat.whom }.into());
        };
        let mut delete = Query::delete()
            .from_table(table::Member::Table)
            .and_where(Expr::col(table::Member::OrgId).eq(seat.org.to_string()))
            .and_where(Expr::col(table::Member::AccountId).eq(seat.whom.to_string()))
            .and_where(Expr::col(table::Member::Role).eq(had.name()))
            .and_where(Self::stays(seat, Role::Reader))
            .to_owned();
        if seat.by != seat.whom {
            delete.and_where(Expr::exists(Self::managing(seat.org, seat.by, &[had])));
        }
        if self.ports.db().run(delete.into()).await?.changes != 1 {
            return Err(self.refused_change(seat, &[had]).await?);
        }
        Ok(())
    }

    /// The keys of the organization, each with whether the member who approved it still is one.
    pub async fn org_keys(&self, org: Id<AccountKind>) -> Result<Vec<OrgKey>, Error<P>> {
        self.org(org).await?.ok_or(Refusal::NoOrg { org })?;
        let keys = Keys::new(self.ports.db(), crate::keys::Settings::default());
        let mut listed = Vec::new();
        for key in keys.list(org).await? {
            let creator_removed = self.role(org, key.approved_by).await?.is_none();
            listed.push(OrgKey { key, creator_removed });
        }
        Ok(listed)
    }

    /// Approves an enrolled deploy key for a site of the organization, on behalf of a member
    /// whose role allows it. The key belongs to the organization, so it outlives the member, and
    /// it has no more than the member's role gives.
    pub async fn approve_for(&self, org: Id<AccountKind>, approval: Approval, at: Timestamp) -> Result<Key, Error<P>> {
        let role = self.role(org, approval.by).await?.ok_or(Refusal::NotMember { org, account: approval.by })?;
        let allowed = role.rights();
        if !allowed.has(Right::Deploy) {
            return Err(Refusal::Role { org, role, needs: Right::Deploy }.into());
        }
        let within = approval.within.both(allowed);
        let keys = Keys::new(self.ports.db(), crate::keys::Settings::default());
        Ok(keys.approve_as(Approval { within, ..approval }, org, at).await?)
    }

    /// The `SELECT` of the seat `account` has in `org`.
    fn seat_of(org: Id<AccountKind>, account: Id<AccountKind>) -> SelectStatement {
        Query::select()
            .from(table::Member::Table)
            .and_where(Expr::col(table::Member::OrgId).eq(org.to_string()))
            .and_where(Expr::col(table::Member::AccountId).eq(account.to_string()))
            .to_owned()
    }

    /// The `SELECT` that has a row when `by` is a member whose role manages every role of `touched`.
    fn managing(org: Id<AccountKind>, by: Id<AccountKind>, touched: &[Role]) -> SelectStatement {
        let allowed = touched
            .iter()
            .map(|role| Role::managers(*role))
            .reduce(|left, right| left.into_iter().filter(|role| right.contains(role)).collect())
            .unwrap_or_default();
        Self::seat_of(org, by)
            .expr(Expr::val(1_i64))
            .and_where(Expr::col(table::Member::Role).is_in(allowed.iter().map(|role| role.name())))
            .to_owned()
    }

    /// The condition that the change leaves an owner: it holds when the seat is no owner's, when
    /// the seat stays an owner's, or when another owner exists.
    fn stays(seat: Seat, becomes: Role) -> sea_query::SimpleExpr {
        if becomes == Role::Owner {
            return Expr::val(1_i64).eq(1_i64);
        }
        let other = Query::select()
            .expr(Expr::val(1_i64))
            .from(table::Member::Table)
            .and_where(Expr::col(table::Member::OrgId).eq(seat.org.to_string()))
            .and_where(Expr::col(table::Member::Role).eq(Role::Owner.name()))
            .and_where(Expr::col(table::Member::AccountId).ne(seat.whom.to_string()))
            .to_owned();
        Expr::col(table::Member::Role).ne(Role::Owner.name()).or(Expr::exists(other))
    }

    /// Why a change that touched no row was refused: the member is gone, the asker has no seat or
    /// the wrong one, or the member is the last owner.
    async fn refused_change(&self, seat: Seat, touched: &[Role]) -> Result<Error<P>, Error<P>> {
        let Some(now) = self.role(seat.org, seat.whom).await? else {
            return Ok(Refusal::NotMember { org: seat.org, account: seat.whom }.into());
        };
        let allowed = seat.by == seat.whom && touched.len() == 1;
        if !allowed && !self.manages(seat, touched).await? {
            return self.denied(seat).await;
        }
        if now == Role::Owner {
            return Ok(Refusal::LastOwner { org: seat.org }.into());
        }
        Ok(Refusal::NotMember { org: seat.org, account: seat.whom }.into())
    }

    async fn manages(&self, seat: Seat, touched: &[Role]) -> Result<bool, Error<P>> {
        let Some(role) = self.role(seat.org, seat.by).await? else { return Ok(false) };
        Ok(touched.iter().all(|other| role.manages(*other)))
    }

    /// The refusal for an asker who may not manage `touched`.
    async fn denied(&self, seat: Seat) -> Result<Error<P>, Error<P>> {
        let Some(role) = self.role(seat.org, seat.by).await? else {
            return Ok(Refusal::NotMember { org: seat.org, account: seat.by }.into());
        };
        Ok(Refusal::Role { org: seat.org, role, needs: Right::Members }.into())
    }
}
