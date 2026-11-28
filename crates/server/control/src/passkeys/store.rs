//! The passkey store, over the row port alone: credentials, the ceremonies in progress, and the
//! relying party id they were made under.

use std::time::Duration;

use db::{Constraint, Db};
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::Offset;
use sea_query::{Expr, ExprTrait, OnConflict, Order, Query};

use super::{Audit, CredentialId, Enrolled, Error, Refused, State, Transport, Verified};
use crate::fields::Fields;
use crate::id::{Account as AccountKind, Factor as FactorKind, Id};
use crate::keys::Label;
use crate::signin::{Context, Proof};
use crate::table;
use crate::unreadable::Unreadable;

/// What a ceremony is for. A registration's reply cannot complete a sign-in, and the reverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Flow {
    /// A person adds a passkey to the account they are signed in to.
    Register,
    /// A person signs in with a passkey.
    Login,
}

impl Flow {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 2] = [("register", Self::Register), ("login", Self::Login)];

    /// The name it is stored as.
    pub fn name(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Login => "login",
        }
    }
}

/// A passkey of an account, as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Passkey {
    /// The row's id, which names it in a list and in a session's link.
    pub id: Id<FactorKind>,
    /// The account.
    pub account: Id<AccountKind>,
    /// The credential, as it is verified against.
    pub stored: super::Stored,
    /// The name the person gave it.
    pub label: Label,
    /// When it was registered.
    pub created: Timestamp,
    /// The day it last signed in, never finer.
    pub used: Option<Date>,
}

impl Passkey {
    /// The columns a read selects.
    const COLUMNS: [table::Passkey; 11] = [
        table::Passkey::Id,
        table::Passkey::AccountId,
        table::Passkey::CredentialId,
        table::Passkey::PublicKey,
        table::Passkey::SignCount,
        table::Passkey::BackupEligible,
        table::Passkey::BackupState,
        table::Passkey::Transports,
        table::Passkey::Label,
        table::Passkey::CreatedAt,
        table::Passkey::LastUsedOn,
    ];
}

impl TryFrom<&db::Row> for Passkey {
    type Error = Unreadable;

    fn try_from(row: &db::Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let counter = u32::try_from(fields.count(table::Passkey::SignCount)?)
            .map_err(|source| Unreadable::Parsed { column: "sign_count", source: Box::new(source) })?;
        let transports = fields.text(table::Passkey::Transports)?;
        let label =
            Label::parse(&fields.text(table::Passkey::Label)?)
            .map_err(|source| Unreadable::Parsed { column: "label", source: Box::new(source) })?;
        Ok(Self {
            id: fields.id(table::Passkey::Id)?,
            account: fields.id(table::Passkey::AccountId)?,
            stored: super::Stored {
                credential: CredentialId::new(fields.bytes(table::Passkey::CredentialId)?),
                key: super::Bytes::new(fields.bytes(table::Passkey::PublicKey)?),
                counter,
                eligible: fields.flag(table::Passkey::BackupEligible)?,
                backed_up: fields.flag(table::Passkey::BackupState)?,
                transports: transports.split(',').filter(|name| !name.is_empty()).map(Transport::named).collect(),
            },
            label,
            created: fields.time(table::Passkey::CreatedAt)?,
            used: fields.maybe_day(table::Passkey::LastUsedOn)?,
        })
    }
}

/// A passkey to add: what a registration verified, and the row id and name to give it.
#[derive(Debug)]
pub struct Fresh<'a> {
    /// The row's id, drawn by the caller.
    pub id: Id<FactorKind>,
    /// The account.
    pub account: Id<AccountKind>,
    /// What the registration verified.
    pub enrolled: &'a Enrolled,
    /// The name the person gave it.
    pub label: &'a Label,
}

impl Fresh<'_> {
    /// The insert of its row.
    fn row(&self, now: Timestamp) -> Result<sea_query::InsertStatement, sea_query::error::Error> {
        let Self { id, account, enrolled, label } = self;
        let transports: Vec<&str> = enrolled.transports.iter().map(|transport| transport.name()).collect();
        Ok(Query::insert()
            .into_table(table::Passkey::Table)
            .columns([
                table::Passkey::Id,
                table::Passkey::AccountId,
                table::Passkey::CredentialId,
                table::Passkey::PublicKey,
                table::Passkey::SignCount,
                table::Passkey::BackupEligible,
                table::Passkey::BackupState,
                table::Passkey::Transports,
                table::Passkey::Label,
                table::Passkey::CreatedAt,
            ])
            .values([
                Expr::val(id.to_string()),
                Expr::val(account.to_string()),
                Expr::val(enrolled.credential.as_slice()),
                Expr::val(enrolled.key.as_slice()),
                Expr::val(i64::from(enrolled.counter)),
                Expr::val(enrolled.eligible),
                Expr::val(enrolled.backed_up),
                Expr::val(transports.join(",")),
                Expr::val(label.as_str()),
                Expr::val(now.as_millisecond()),
            ])?
            .to_owned())
    }
}

/// A ceremony to keep.
#[derive(Clone, Copy, Debug)]
pub struct Begin<'a> {
    /// What it is for.
    pub flow: Flow,
    /// The account, for a registration; a sign-in does not know yet.
    pub account: Option<Id<AccountKind>>,
    /// The verifier's state.
    pub state: &'a State,
    /// The secret that binds it to a browser: the value of the ceremony cookie.
    pub proof: &'a Proof,
}

/// A ceremony that was found and spent.
#[derive(Debug)]
pub struct Spent {
    /// The account it was started for.
    pub account: Option<Id<AccountKind>>,
    /// The verifier's state.
    pub state: State,
}

/// The passkey store over a database.
///
/// A ceremony is kept as the digest of its cookie secret, single use: [`Registry::spend`] deletes
/// the row by a conditional statement that one caller wins, so a reply cannot be replayed, and it
/// ends after [`Registry::LIFE`] whether or not the library has an opinion. The caller draws every
/// id and secret and says what time it is.
#[derive(Clone, Debug)]
pub struct Registry<D> {
    db: D,
}

impl<D: Db> Registry<D> {
    /// How long a ceremony lasts: five minutes.
    pub const LIFE: Duration = Duration::from_secs(300);
    /// The `auth_config` row's key.
    const SLOT: &'static str = "relying-party";

    /// A store over `db`.
    pub const fn new(db: D) -> Self {
        Self { db }
    }

    /// The recovery codes of the same database.
    pub fn codes(&self) -> crate::recovery::Codes<D>
    where
        D: Clone,
    {
        crate::recovery::Codes::new(self.db.clone())
    }

    fn day(now: Timestamp) -> Date {
        Offset::UTC.to_datetime(now).date()
    }

    /// Refuses with [`Refused::Moved`] when passkeys exist that were made under another relying
    /// party id than `rp_id`: they would not work, and the person is told instead of failing at
    /// the authenticator.
    pub async fn check(&self, rp_id: &str) -> Result<(), Error<D::Error>> {
        let select = Query::select()
            .column(table::AuthConfig::RpId)
            .from(table::AuthConfig::Table)
            .and_where(Expr::col(table::AuthConfig::Slot).eq(Self::SLOT))
            .to_owned();
        let Some(row) = self.db.first(select.into()).await? else { return Ok(()) };
        let was = Fields(&row).text(table::AuthConfig::RpId)?;
        if was == rp_id {
            return Ok(());
        }
        Err(Refused::Moved { was, now: rp_id.to_owned(), passkeys: self.count().await? }.into())
    }

    /// How many passkeys there are, over every account.
    async fn count(&self) -> Result<u64, Error<D::Error>> {
        let select = Query::select()
            .expr_as(Expr::col(table::Passkey::Id).count(), table::Computed::Count)
            .from(table::Passkey::Table)
            .to_owned();
        let row = self.db.first(select.into()).await?;
        Ok(row.map(|row| Fields(&row).count(table::Computed::Count)).transpose()?.unwrap_or(0))
    }

    /// Keeps a ceremony, and sweeps the ended ones.
    pub async fn begin(&self, begin: Begin<'_>, now: Timestamp) -> Result<(), Error<D::Error>> {
        let sweep = Query::delete()
            .from_table(table::Ceremony::Table)
            .and_where(Expr::col(table::Ceremony::ExpiresAt).lt(now.as_millisecond()))
            .to_owned();
        let ends = now.checked_add(Self::LIFE)?;
        let insert = Query::insert()
            .into_table(table::Ceremony::Table)
            .columns([
                table::Ceremony::Digest,
                table::Ceremony::Flow,
                table::Ceremony::AccountId,
                table::Ceremony::State,
                table::Ceremony::ExpiresAt,
            ])
            .values([
                Expr::val(Context::Ceremony.digest(begin.proof).as_bytes().as_slice()),
                Expr::val(begin.flow.name()),
                Expr::val(begin.account.map(|account| account.to_string())),
                Expr::val(begin.state.as_slice()),
                Expr::val(ends.as_millisecond()),
            ])?
            .to_owned();
        self.db.batch(vec![sweep.into(), insert.into()]).await?;
        Ok(())
    }

    /// Finds the ceremony a cookie secret names and spends it: `None` when there is none, it is
    /// for another flow, it ended, or it was spent (the four look the same). A replay finds
    /// nothing, because the row is gone after the first reply, right or wrong.
    pub async fn spend(&self, proof: &Proof, flow: Flow, now: Timestamp) -> Result<Option<Spent>, Error<D::Error>> {
        let digest = Context::Ceremony.digest(proof);
        let select = Query::select()
            .columns([
                table::Ceremony::Digest,
                table::Ceremony::Flow,
                table::Ceremony::AccountId,
                table::Ceremony::State,
            ])
            .from(table::Ceremony::Table)
            .and_where(Expr::col(table::Ceremony::Digest).eq(digest.as_bytes().as_slice()))
            .and_where(Expr::col(table::Ceremony::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        let Some(row) = self.db.first(select.into()).await? else { return Ok(None) };
        let fields = Fields(&row);
        if fields.digest(table::Ceremony::Digest)? != digest {
            return Ok(None);
        }
        let found = fields.name(table::Ceremony::Flow, &Flow::NAMES)?;
        let spend = Query::delete()
            .from_table(table::Ceremony::Table)
            .and_where(Expr::col(table::Ceremony::Digest).eq(digest.as_bytes().as_slice()))
            .and_where(Expr::col(table::Ceremony::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        // spent even when it is for another flow: a reply sent to the wrong endpoint ends it
        if self.db.run(spend.into()).await?.changes != 1 || found != flow {
            return Ok(None);
        }
        Ok(Some(Spent {
            account: fields.maybe_id(table::Ceremony::AccountId)?,
            state: State::new(fields.bytes(table::Ceremony::State)?),
        }))
    }

    /// Adds a passkey, with its event, and records the relying party id it is bound to when it
    /// is the first. [`Refused::Taken`] if the credential is already registered.
    pub async fn add(&self, fresh: Fresh<'_>, rp_id: &str, now: Timestamp) -> Result<(), Error<D::Error>> {
        let bind = Query::insert()
            .into_table(table::AuthConfig::Table)
            .columns([table::AuthConfig::Slot, table::AuthConfig::RpId])
            .values([Expr::val(Self::SLOT), Expr::val(rp_id)])?
            .on_conflict(OnConflict::column(table::AuthConfig::Slot).do_nothing().to_owned())
            .to_owned();
        let audit = Audit::Added { passkey: fresh.id }.statement(fresh.account, now)?;
        match self.db.batch(vec![fresh.row(now)?.into(), bind.into(), audit]).await {
            Ok(_) => Ok(()),
            Err(error) if error.constraint() == Some(Constraint::Unique) => Err(Refused::Taken.into()),
            Err(error) => Err(error.into()),
        }
    }

    /// The passkey of this credential id, if there is one.
    pub async fn find(&self, credential: &CredentialId) -> Result<Option<Passkey>, Error<D::Error>> {
        let select = Query::select()
            .columns(Passkey::COLUMNS)
            .from(table::Passkey::Table)
            .and_where(Expr::col(table::Passkey::CredentialId).eq(credential.as_slice()))
            .to_owned();
        let row = self.db.first(select.into()).await?;
        Ok(row.map(|row| Passkey::try_from(&row)).transpose()?)
    }

    /// The passkeys of `account`, oldest first.
    pub async fn list(&self, account: Id<AccountKind>) -> Result<Vec<Passkey>, Error<D::Error>> {
        let select = Query::select()
            .columns(Passkey::COLUMNS)
            .from(table::Passkey::Table)
            .and_where(Expr::col(table::Passkey::AccountId).eq(account.to_string()))
            .order_by(table::Passkey::CreatedAt, Order::Asc)
            .order_by(table::Passkey::Id, Order::Asc)
            .to_owned();
        let rows = self.db.rows(select.into()).await?;
        Ok(rows.iter().map(Passkey::try_from).collect::<Result<_, _>>()?)
    }

    /// Writes what a verified sign-in changed: the counter and backup state, the day, and the
    /// events. The counter is written only if it is still what `passkey` read, so two sign-ins
    /// that race on one passkey cannot both win ([`Refused::Raced`]).
    pub async fn used(&self, passkey: &Passkey, verified: &Verified, now: Timestamp) -> Result<(), Error<D::Error>> {
        let update = Query::update()
            .table(table::Passkey::Table)
            .value(table::Passkey::SignCount, i64::from(Ord::max(verified.counter, passkey.stored.counter)))
            .value(table::Passkey::BackupState, verified.backed_up)
            .value(table::Passkey::LastUsedOn, Self::day(now).to_string())
            .and_where(Expr::col(table::Passkey::Id).eq(passkey.id.to_string()))
            .and_where(Expr::col(table::Passkey::SignCount).eq(i64::from(passkey.stored.counter)))
            .to_owned();
        let mut batch = vec![update.into()];
        if verified.judged == super::Counter::Flagged {
            batch.push(Audit::Flagged { passkey: passkey.id, refused: false }.statement(passkey.account, now)?);
        }
        let outcomes = self.db.batch(batch).await?;
        if outcomes.first().map(|outcome| outcome.changes) != Some(1) {
            return Err(Refused::Raced.into());
        }
        Ok(())
    }

    /// Writes an event for `passkey`: a refused counter, or a sign-in.
    pub async fn record(&self, passkey: &Passkey, audit: Audit, now: Timestamp) -> Result<(), Error<D::Error>> {
        self.db.run(audit.statement(passkey.account, now)?).await?;
        Ok(())
    }

    /// Removes a passkey of `account`, ends the sessions it made, and writes the event, in one
    /// batch. [`Refused::Missing`] when the account has no such passkey, which is the answer for
    /// another account's passkey too.
    pub async fn remove(
        &self,
        account: Id<AccountKind>,
        id: Id<FactorKind>,
        now: Timestamp,
    ) -> Result<(), Error<D::Error>> {
        let found = Query::select()
            .expr(Expr::val(1_i64))
            .from(table::Passkey::Table)
            .and_where(Expr::col(table::Passkey::Id).eq(id.to_string()))
            .and_where(Expr::col(table::Passkey::AccountId).eq(account.to_string()))
            .to_owned();
        if self.db.first(found.into()).await?.is_none() {
            return Err(Refused::Missing.into());
        }
        let made = Query::select()
            .column(table::PasskeySession::SessionId)
            .from(table::PasskeySession::Table)
            .and_where(Expr::col(table::PasskeySession::PasskeyId).eq(id.to_string()))
            .to_owned();
        let end = Query::update()
            .table(table::Session::Table)
            .value(table::Session::RevokedAt, now.as_millisecond())
            .and_where(Expr::col(table::Session::RevokedAt).is_null())
            .and_where(ExprTrait::in_subquery(Expr::col(table::Session::Id), made))
            .to_owned();
        let delete = Query::delete()
            .from_table(table::Passkey::Table)
            .and_where(Expr::col(table::Passkey::Id).eq(id.to_string()))
            .and_where(Expr::col(table::Passkey::AccountId).eq(account.to_string()))
            .to_owned();
        let audit = Audit::Removed { passkey: id }.statement(account, now)?;
        self.db.batch(vec![end.into(), delete.into(), audit]).await?;
        Ok(())
    }
}
