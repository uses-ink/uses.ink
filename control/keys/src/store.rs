//! The key use cases, over the row port alone.

use db::{Constraint, Db, Statement};
use jiff::Timestamp;
use jiff::civil::Date;
use sea_query::{Expr, ExprTrait, Order, Query};
use sig::{Fingerprint, KeyId};

use super::audit::{Audit, Entry};
use super::enrol::{Enrolment, NewEnrolment, Poll, Seen, State, UserCode};
use super::error::{Error, Refused};
use super::key::{Key, Kind, Why};
use super::right::{Rights, Scope};
use control_rows::id::{Account as AccountKind, Enrolment as EnrolmentKind, Id};
use control_signin::Ttl;
use control_rows::table;

/// What the operator decides about keys: how long each lives, the ceiling for a request, how
/// long an ask waits for a person, and how long a finished ask is kept for its poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// A device key's life, when none is asked: 180 days.
    pub device: Ttl,
    /// A deploy key's life, when none is asked: 90 days.
    pub deploy: Ttl,
    /// The longest a key may be asked to live: 365 days. A request above it is refused naming
    /// both numbers.
    pub ceiling: Ttl,
    /// How long a person has to approve: 10 minutes.
    pub enrol: Ttl,
    /// How long a decided or expired ask stays for its poll to read: 1 hour.
    pub kept: Ttl,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device: Ttl::secs(180 * 86_400),
            deploy: Ttl::secs(90 * 86_400),
            ceiling: Ttl::secs(365 * 86_400),
            enrol: Ttl::secs(10 * 60),
            kept: Ttl::secs(60 * 60),
        }
    }
}

/// An approval: the code the person typed, the fingerprint they say they compared, who they
/// are and the most they may give.
#[derive(Clone, Debug)]
pub struct Approval {
    /// The code.
    pub code: UserCode,
    /// The fingerprint the person read on both screens.
    pub fingerprint: Fingerprint,
    /// The account of the person who approves.
    pub by: Id<AccountKind>,
    /// The rights the person has. A key cannot have more.
    pub within: Rights,
}

/// Keys, enrolments and the credential audit over a database.
///
/// The caller says what time it is and draws every code and id, so no call reads a clock or
/// random numbers. What must hold at once is in a statement's `WHERE`: an enrolment is approved
/// by a conditional update that only one caller wins, and the key and the event are written by
/// statements that exist only if that update changed the row.
#[derive(Clone, Debug)]
pub struct Keys<D> {
    db: D,
    settings: Settings,
}

impl<D: Db> Keys<D> {
    /// Keys on `db`, under `settings`.
    pub const fn new(db: D, settings: Settings) -> Self {
        Self { db, settings }
    }

    /// The settings it runs under.
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The day `now` falls on, in UTC: the finest "last used" ever is.
    pub fn day(now: Timestamp) -> Date {
        jiff::tz::Offset::UTC.to_datetime(now).date()
    }

    /// Makes a pending enrolment, after sweeping ones that ended. Asking again with the same key
    /// replaces its earlier ask. A key that is already enrolled is refused.
    #[expect(clippy::too_many_lines, reason = "one transaction, its statements read top to bottom")]
    pub async fn enrol(&self, new: NewEnrolment) -> Result<Enrolment, Error<D::Error>> {
        let lifetime = new.lifetime.unwrap_or(match new.purpose {
            Kind::Device => self.settings.device,
            Kind::Deploy => self.settings.deploy,
        });
        if lifetime > self.settings.ceiling {
            return Err(Refused::Lifetime { asked: lifetime, ceiling: self.settings.ceiling }.into());
        }
        match (new.purpose, new.site) {
            (Kind::Deploy, None) => return Err(Refused::Site.into()),
            (Kind::Device, Some(_)) => return Err(Refused::Kind { kind: Kind::Device }.into()),
            _ => {}
        }
        let rights = if new.rights.is_empty() {
            match new.purpose {
                Kind::Device => Rights::DEVICE,
                Kind::Deploy => Rights::DEPLOY,
            }
        } else {
            new.rights
        };
        let key = new.public.id(new.key_type);
        if self.key(key).await?.is_some() {
            return Err(Refused::KeyTaken.into());
        }
        if self.by_code(&new.code).await?.is_some_and(|held| held.expires > new.at) {
            return Err(Refused::CodeTaken.into());
        }
        let expires = self.settings.enrol.after(new.at)?;
        let row = Enrolment {
            id: new.id,
            code: new.code,
            key_type: new.key_type,
            public: new.public,
            label: new.label,
            purpose: new.purpose,
            site: new.site,
            rights,
            lifetime,
            state: State::Pending,
            created: new.at,
            expires,
        };
        let swept = self.settings.kept.duration();
        let before = new.at.as_millisecond().saturating_sub(i64::try_from(swept.as_millis()).unwrap_or(i64::MAX));
        let sweep = Query::delete()
            .from_table(table::Enrolment::Table)
            .and_where(Expr::col(table::Enrolment::ExpiresAt).lt(before))
            .to_owned();
        let replace = Query::delete()
            .from_table(table::Enrolment::Table)
            .and_where(Expr::col(table::Enrolment::KeyId).eq(key.to_string()))
            .and_where(Expr::col(table::Enrolment::State).ne(State::Approved.name()))
            .to_owned();
        let stale = Query::delete()
            .from_table(table::Enrolment::Table)
            .and_where(Expr::col(table::Enrolment::UserCode).eq(row.code.as_str().to_owned()))
            .and_where(Expr::col(table::Enrolment::ExpiresAt).lte(new.at.as_millisecond()))
            .to_owned();
        let insert = Query::insert()
            .into_table(table::Enrolment::Table)
            .columns([
                table::Enrolment::Id,
                table::Enrolment::UserCode,
                table::Enrolment::KeyId,
                table::Enrolment::KeyType,
                table::Enrolment::PublicKey,
                table::Enrolment::Label,
                table::Enrolment::Purpose,
                table::Enrolment::SiteId,
                table::Enrolment::Rights,
                table::Enrolment::Lifetime,
                table::Enrolment::State,
                table::Enrolment::CreatedAt,
                table::Enrolment::ExpiresAt,
            ])
            .values([
                Expr::val(row.id.to_string()),
                Expr::val(row.code.as_str().to_owned()),
                Expr::val(key.to_string()),
                Expr::val(row.key_type.name()),
                Expr::val(row.public.bytes().as_slice()),
                Expr::val(row.label.as_str().to_owned()),
                Expr::val(row.purpose.name()),
                Expr::val(row.site.map(|site| site.to_string())),
                Expr::val(row.rights.stored()),
                Expr::val(i64::try_from(lifetime.duration().as_secs()).unwrap_or(i64::MAX)),
                Expr::val(State::Pending.name()),
                Expr::val(row.created.as_millisecond()),
                Expr::val(row.expires.as_millisecond()),
            ])?
            .to_owned();
        match self.db.batch(vec![sweep.into(), replace.into(), stale.into(), insert.into()]).await {
            Ok(_) => Ok(row),
            Err(error) if error.constraint() == Some(Constraint::Unique) => Err(Refused::KeyTaken.into()),
            Err(error) => Err(error.into()),
        }
    }

    /// How many enrolments are waiting for a person at `now`.
    pub async fn waiting(&self, now: Timestamp) -> Result<u64, Error<D::Error>> {
        let select = Query::select()
            .expr_as(Expr::col(table::Enrolment::Id).count(), table::Computed::Count)
            .from(table::Enrolment::Table)
            .and_where(Expr::col(table::Enrolment::State).eq(State::Pending.name()))
            .and_where(Expr::col(table::Enrolment::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        let row = self.db.first(select.into()).await?;
        Ok(row.map_or(Ok(0), |row| row.get::<i64>(table::Computed::Count).map(|n| u64::try_from(n).unwrap_or(0)))?)
    }

    async fn select_enrolment(&self, filter: Expr) -> Result<Option<Enrolment>, Error<D::Error>> {
        let select =
            Query::select().columns(Enrolment::COLUMNS).from(table::Enrolment::Table).and_where(filter).to_owned();
        let row = self.db.first(select.into()).await?;
        Ok(row.map(|row| Enrolment::try_from(&row)).transpose()?)
    }

    async fn by_code(&self, code: &UserCode) -> Result<Option<Enrolment>, Error<D::Error>> {
        self.select_enrolment(Expr::col(table::Enrolment::UserCode).eq(code.as_str().to_owned())).await
    }

    /// The enrolment of `id`, and what a poll answers about it at `now`.
    pub async fn poll(
        &self,
        id: Id<EnrolmentKind>,
        now: Timestamp,
    ) -> Result<Option<(Enrolment, Poll)>, Error<D::Error>> {
        let Some(found) = self.select_enrolment(Expr::col(table::Enrolment::Id).eq(id.to_string())).await? else {
            return Ok(None);
        };
        let approved = match found.state {
            State::Approved => self.key(found.key()).await?.map(|key| key.expires),
            _ => None,
        };
        let poll = found.poll(approved, now);
        Ok(Some((found, poll)))
    }

    /// The enrolment whose key is `key`, in any state: the one place a key that is not yet
    /// enrolled can sign, to read its own outcome.
    pub async fn waiting_key(&self, key: KeyId) -> Result<Option<Enrolment>, Error<D::Error>> {
        self.select_enrolment(Expr::col(table::Enrolment::KeyId).eq(key.to_string())).await
    }

    /// What a person sees before approving: the enrolment under `code`, when it is pending and
    /// has not ended. An unknown, decided and expired code are the same `None`.
    pub async fn look(&self, code: &UserCode, now: Timestamp) -> Result<Option<Seen>, Error<D::Error>> {
        let found = self.by_code(code).await?;
        Ok(found.filter(|found| found.state == State::Pending && found.expires > now).map(|found| Seen {
            fingerprint: found.fingerprint(),
            label: found.label,
            key_type: found.key_type,
            purpose: found.purpose,
            site: found.site,
            rights: found.rights,
            lifetime: found.lifetime,
            asked: found.created,
        }))
    }

    /// Approves the enrolment under the code: the key goes live and an event is written, or
    /// nothing changes.
    #[expect(clippy::too_many_lines, reason = "one transaction, its statements read top to bottom")]
    pub async fn approve(&self, approval: Approval, now: Timestamp) -> Result<Key, Error<D::Error>> {
        let found = self.by_code(&approval.code).await?;
        let found =
            found.filter(|found| found.state == State::Pending && found.expires > now).ok_or(Refused::NoEnrolment)?;
        if found.fingerprint() != approval.fingerprint {
            return Err(Refused::Fingerprint.into());
        }
        if !approval.within.covers(found.rights) {
            return Err(Refused::Rights.into());
        }
        let scope = match (found.purpose, found.site) {
            (Kind::Device, _) => Scope::Account(approval.by),
            (Kind::Deploy, Some(site)) => {
                self.owns(approval.by, site).await?;
                Scope::Site(site)
            }
            (Kind::Deploy, None) => return Err(Refused::Site.into()),
        };
        let key = Key {
            id: found.key(),
            account: approval.by,
            kind: found.purpose,
            key_type: found.key_type,
            public: found.public,
            label: found.label.clone(),
            grant: super::right::Grant { scope, rights: found.rights },
            created: now,
            approved_by: approval.by,
            expires: found.lifetime.after(now)?,
            last_used: None,
            revoked: None,
        };
        let decide = Query::update()
            .table(table::Enrolment::Table)
            .value(table::Enrolment::State, State::Approved.name())
            .value(table::Enrolment::ApprovedBy, approval.by.to_string())
            .value(table::Enrolment::ApprovedAt, now.as_millisecond())
            .and_where(Expr::col(table::Enrolment::Id).eq(found.id.to_string()))
            .and_where(Expr::col(table::Enrolment::State).eq(State::Pending.name()))
            .and_where(Expr::col(table::Enrolment::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        let decided = Query::select()
            .expr(Expr::val(1_i64))
            .from(table::Enrolment::Table)
            .and_where(Expr::col(table::Enrolment::Id).eq(found.id.to_string()))
            .and_where(Expr::col(table::Enrolment::State).eq(State::Approved.name()))
            .and_where(Expr::col(table::Enrolment::ApprovedAt).eq(now.as_millisecond()))
            .and_where(Expr::col(table::Enrolment::ApprovedBy).eq(approval.by.to_string()))
            .to_owned();
        let audit = Audit::KeyApproved { key: key.id, kind: key.kind, rights: found.rights, scope };
        let statements = vec![
            decide.into(),
            Self::insert_key(&key, &decided)?,
            Self::guarded(approval.by, &audit, now, Expr::exists(decided))?,
        ];
        let outcomes = self.db.batch(statements).await?;
        if outcomes.first().map(|outcome| outcome.changes) != Some(1) {
            return Err(Refused::NoEnrolment.into());
        }
        Ok(key)
    }

    /// Turns the enrolment under `code` down. The same refusal as for an unknown code when
    /// there is none to turn down.
    pub async fn deny(&self, code: &UserCode, now: Timestamp) -> Result<(), Error<D::Error>> {
        let deny = Query::update()
            .table(table::Enrolment::Table)
            .value(table::Enrolment::State, State::Denied.name())
            .and_where(Expr::col(table::Enrolment::UserCode).eq(code.as_str().to_owned()))
            .and_where(Expr::col(table::Enrolment::State).eq(State::Pending.name()))
            .and_where(Expr::col(table::Enrolment::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        if self.db.run(deny.into()).await?.changes == 1 { Ok(()) } else { Err(Refused::NoEnrolment.into()) }
    }

    /// Whether `site` is one of `account`'s.
    async fn owns(&self, account: Id<AccountKind>, site: Id<control_rows::id::Site>) -> Result<(), Error<D::Error>> {
        let select = Query::select()
            .expr(Expr::val(1_i64))
            .from(table::Site::Table)
            .and_where(Expr::col(table::Site::Id).eq(site.to_string()))
            .and_where(Expr::col(table::Site::AccountId).eq(account.to_string()))
            .limit(1)
            .to_owned();
        if self.db.first(select.into()).await?.is_some() { Ok(()) } else { Err(Refused::Site.into()) }
    }

    /// The statement that writes `key`, only if `guard` holds.
    fn insert_key(key: &Key, guard: &sea_query::SelectStatement) -> Result<Statement, Error<D::Error>> {
        let (scope_kind, site) = Key::scope_columns(key.grant.scope);
        let mut select = Query::select();
        select
            .exprs([
                Expr::val(key.id.to_string()),
                Expr::val(key.account.to_string()),
                Expr::val(key.kind.name().to_owned()),
                Expr::val(key.key_type.name().to_owned()),
                Expr::val(key.public.bytes().to_vec()),
                Expr::val(key.label.as_str().to_owned()),
                Expr::val(key.grant.rights.stored()),
                Expr::val(scope_kind.to_owned()),
            ])
            .expr(Expr::val(site))
            .exprs([
                Expr::val(key.created.as_millisecond()),
                Expr::val(key.approved_by.to_string()),
                Expr::val(key.expires.as_millisecond()),
            ])
            .and_where(Expr::exists(guard.clone()));
        let mut insert = Query::insert();
        insert
            .into_table(table::Key::Table)
            .columns([
                table::Key::Id,
                table::Key::AccountId,
                table::Key::Kind,
                table::Key::KeyType,
                table::Key::PublicKey,
                table::Key::Label,
                table::Key::Rights,
                table::Key::ScopeKind,
                table::Key::SiteId,
                table::Key::CreatedAt,
                table::Key::ApprovedBy,
                table::Key::ExpiresAt,
            ])
            .select_from(select)?;
        Ok(insert.into())
    }

    /// The statement that writes `audit` for `account`, only if `guard` holds.
    fn guarded(
        account: Id<AccountKind>,
        audit: &Audit,
        now: Timestamp,
        guard: Expr,
    ) -> Result<Statement, Error<D::Error>> {
        let mut select = Query::select();
        select
            .exprs([
                Expr::val(account.to_string()),
                Expr::val(audit.key().to_string()),
                Expr::val(now.as_millisecond()),
                Expr::val(audit.name()),
                Expr::val(audit.body()?),
            ])
            .and_where(guard);
        let mut insert = Query::insert();
        insert
            .into_table(table::AuthEvent::Table)
            .columns([
                table::AuthEvent::AccountId,
                table::AuthEvent::KeyId,
                table::AuthEvent::At,
                table::AuthEvent::Name,
                table::AuthEvent::Body,
            ])
            .select_from(select)?;
        Ok(insert.into())
    }

    /// The key with this id, when there is one.
    pub async fn key(&self, id: KeyId) -> Result<Option<Key>, Error<D::Error>> {
        let select = Query::select()
            .columns(Key::COLUMNS)
            .from(table::Key::Table)
            .and_where(Expr::col(table::Key::Id).eq(id.to_string()))
            .to_owned();
        let row = self.db.first(select.into()).await?;
        Ok(row.map(|row| Key::try_from(&row)).transpose()?)
    }

    /// The keys of `account`, oldest first, revoked and expired ones too.
    pub async fn list(&self, account: Id<AccountKind>) -> Result<Vec<Key>, Error<D::Error>> {
        let select = Query::select()
            .columns(Key::COLUMNS)
            .from(table::Key::Table)
            .and_where(Expr::col(table::Key::AccountId).eq(account.to_string()))
            .order_by(table::Key::CreatedAt, Order::Asc)
            .to_owned();
        let rows = self.db.rows(select.into()).await?;
        Ok(rows.iter().map(Key::try_from).collect::<Result<_, _>>()?)
    }

    /// Revokes the key of `account` with this fingerprint, and writes the event, or changes
    /// nothing. Who may ask is the caller's to decide.
    pub async fn revoke(
        &self,
        whose: (Id<AccountKind>, &Fingerprint),
        why: Why,
        now: Timestamp,
    ) -> Result<Key, Error<D::Error>> {
        let (account, fingerprint) = whose;
        let keys = self.list(account).await?;
        let found = keys.into_iter().find(|key| key.id.fingerprint() == *fingerprint && key.revoked.is_none());
        let key = found.ok_or(Refused::NoKey)?;
        let revoke = Query::update()
            .table(table::Key::Table)
            .value(table::Key::RevokedAt, now.as_millisecond())
            .value(table::Key::RevokedWhy, why.name())
            .and_where(Expr::col(table::Key::Id).eq(key.id.to_string()))
            .and_where(Expr::col(table::Key::RevokedAt).is_null())
            .to_owned();
        let revoked = Query::select()
            .expr(Expr::val(1_i64))
            .from(table::Key::Table)
            .and_where(Expr::col(table::Key::Id).eq(key.id.to_string()))
            .and_where(Expr::col(table::Key::RevokedAt).eq(now.as_millisecond()))
            .to_owned();
        let audit = Audit::KeyRevoked { key: key.id, why };
        let outcomes =
            self.db.batch(vec![revoke.into(), Self::guarded(account, &audit, now, Expr::exists(revoked))?]).await?;
        if outcomes.first().map(|outcome| outcome.changes) != Some(1) {
            return Err(Refused::NoKey.into());
        }
        Ok(Key { revoked: Some((now, why)), ..key })
    }

    /// Notes that `key` signed a request today. Written only when the day changes: at most once
    /// a day per key, so the row is the whole record and no request leaves a trace.
    pub async fn used(&self, key: KeyId, today: Date) -> Result<(), Error<D::Error>> {
        let day = today.to_string();
        let update = Query::update()
            .table(table::Key::Table)
            .value(table::Key::LastUsedOn, day.clone())
            .and_where(Expr::col(table::Key::Id).eq(key.to_string()))
            .and_where(Expr::col(table::Key::LastUsedOn).is_null().or(Expr::col(table::Key::LastUsedOn).lt(day)))
            .to_owned();
        self.db.run(update.into()).await?;
        Ok(())
    }

    /// Writes `audit` for `account`. An event that anyone with the key can repeat is written at
    /// most once an hour per key and kind.
    pub async fn record(&self, account: Id<AccountKind>, audit: Audit, now: Timestamp) -> Result<(), Error<D::Error>> {
        let guard = if audit.hourly() {
            let hour = now.as_millisecond().saturating_sub(3_600_000);
            let recent = Query::select()
                .expr(Expr::val(1_i64))
                .from(table::AuthEvent::Table)
                .and_where(Expr::col(table::AuthEvent::KeyId).eq(audit.key().to_string()))
                .and_where(Expr::col(table::AuthEvent::Name).eq(audit.name()))
                .and_where(Expr::col(table::AuthEvent::At).gt(hour))
                .to_owned();
            Expr::not_exists(recent)
        } else {
            Expr::val(1_i64).eq(1_i64)
        };
        self.db.run(Self::guarded(account, &audit, now, guard)?).await?;
        Ok(())
    }

    /// The newest events of `account`, newest first.
    pub async fn events(&self, account: Id<AccountKind>, limit: u64) -> Result<Vec<Entry>, Error<D::Error>> {
        let select = Query::select()
            .columns([
                table::AuthEvent::AccountId,
                table::AuthEvent::At,
                table::AuthEvent::Name,
                table::AuthEvent::Body,
            ])
            .from(table::AuthEvent::Table)
            .and_where(Expr::col(table::AuthEvent::AccountId).eq(account.to_string()))
            .order_by(table::AuthEvent::Seq, Order::Desc)
            .limit(limit)
            .to_owned();
        let rows = self.db.rows(select.into()).await?;
        Ok(rows.iter().map(Entry::try_from).collect::<Result<_, _>>()?)
    }
}
