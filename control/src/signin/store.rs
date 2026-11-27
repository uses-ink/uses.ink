//! The sign-in use cases, over the row port alone.

use db::{Constraint, Db, Statement};
use jiff::Timestamp;
use mail::Address;
use sea_query::{Expr, ExprTrait, IntoTableRef, Order, Query};

use super::{
    Audit, Context, Digest, Drawn, Entered, Error, Factor, Issued, Passed, Pending, Presented, Proof, Purpose, Refused,
    Requested, Session, Settings, Sort,
};
use crate::fields::Fields;
use crate::id::{Account as AccountKind, Id, Session as SessionKind};
use crate::table;

/// Signing in over a database: one-time links, sessions, the first-run link and the policy
/// that says who may make an account.
///
/// It needs only the row port, so a credential check can hold one without the rest of the
/// control plane. The caller draws every secret and id ([`Drawn`]) and says what time it is, so
/// no call reads a clock or random numbers.
///
/// A link is used by one `UPDATE ... WHERE used_at IS NULL`: of two calls that present the same
/// link, the database lets one change the row and the other changes nothing and is refused. If
/// the rows of the sign-in itself then fail to write, the link is spent and the person asks for
/// another.
#[derive(Clone, Debug)]
pub struct SignIn<D> {
    db: D,
    settings: Settings,
}

/// A one-time token to store.
struct Draft<'a> {
    purpose: Purpose,
    address: Option<&'a Address>,
    account: Option<Id<AccountKind>>,
    slot: Option<&'static str>,
    proof: Proof,
    expires: Timestamp,
}

/// What a sign-in writes besides its session.
struct Making {
    session: Session,
    digest: Digest,
    account: Id<AccountKind>,
    fresh: bool,
    attach: Option<Address>,
    factor: Id<crate::id::Factor>,
    now: Timestamp,
}

/// What a stored link says.
struct Link {
    purpose: Purpose,
    address: Option<Address>,
    account: Option<Id<AccountKind>>,
}

impl<D: Db> SignIn<D> {
    /// The slot the first-run link takes, which only one row can.
    const SLOT: &'static str = "admin";

    /// Sign-in on `db`, under `settings`.
    pub const fn new(db: D, settings: Settings) -> Self {
        Self { db, settings }
    }

    /// The settings it runs under.
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    async fn found(&self, table: impl IntoTableRef, filter: Expr) -> Result<bool, Error<D::Error>> {
        let select = Query::select().expr(Expr::val(1_i64)).from(table).and_where(filter).limit(1).to_owned();
        Ok(self.db.first(select.into()).await?.is_some())
    }

    /// The account that holds a link credential at `address`.
    async fn holder(&self, address: &Address) -> Result<Option<Id<AccountKind>>, Error<D::Error>> {
        let select = Query::select()
            .column(table::Factor::AccountId)
            .from(table::Factor::Table)
            .and_where(Expr::col(table::Factor::Kind).eq(Sort::Link.name()))
            .and_where(Expr::col(table::Factor::Address).eq(address.as_str()))
            .to_owned();
        let row = self.db.first(select.into()).await?;
        Ok(row.map(|row| Fields(&row).id(table::Factor::AccountId)).transpose()?)
    }

    /// Whether the sign-up policy lets an address with no account make one.
    async fn allowed(&self) -> Result<bool, Error<D::Error>> {
        match self.settings.signup {
            super::Signup::Open => Ok(true),
            super::Signup::Closed => Ok(false),
            // the first account: nobody has an account to sign in to and no first-run link was made
            super::Signup::First => {
                let taken = self.found(table::Factor::Table, Expr::col(table::Factor::Id).is_not_null()).await?;
                let issued =
                    self.found(table::Challenge::Table, Expr::col(table::Challenge::Slot).eq(Self::SLOT)).await?;
                Ok(!(taken || issued))
            }
        }
    }

    async fn issue(&self, draft: Draft<'_>, now: Timestamp) -> Result<Issued, Error<D::Error>> {
        let digest = Context::Challenge.digest(&draft.proof);
        let insert = Query::insert()
            .into_table(table::Challenge::Table)
            .columns([
                table::Challenge::Digest,
                table::Challenge::Purpose,
                table::Challenge::Address,
                table::Challenge::AccountId,
                table::Challenge::Slot,
                table::Challenge::CreatedAt,
                table::Challenge::ExpiresAt,
            ])
            .values([
                Expr::val(digest.as_bytes().as_slice()),
                Expr::val(draft.purpose.name()),
                Expr::val(draft.address.map(|address| address.as_str().to_owned())),
                Expr::val(draft.account.map(|account| account.to_string())),
                Expr::val(draft.slot),
                Expr::val(now.as_millisecond()),
                Expr::val(draft.expires.as_millisecond()),
            ])?
            .to_owned();
        self.db.run(insert.into()).await?;
        Ok(Issued { proof: draft.proof, expires: draft.expires, address: draft.address.cloned() })
    }

    /// Makes a sign-in link for `address`, when there is an account at it or the sign-up policy
    /// would make one. A link for an address with no account makes the account when it is used.
    /// Expired rows are swept on the way.
    pub async fn request(&self, address: &Address, drawn: Drawn, now: Timestamp) -> Result<Requested, Error<D::Error>> {
        let account = self.holder(address).await?;
        let wanted = account.is_some() || self.allowed().await?;
        self.purge(now).await?;
        if !wanted {
            return Ok(Requested::Declined);
        }
        let draft = Draft {
            purpose: Purpose::Login,
            address: Some(address),
            account,
            slot: None,
            proof: drawn.proof,
            expires: self.settings.login.after(now)?,
        };
        Ok(Requested::Issued(self.issue(draft, now).await?))
    }

    /// Makes the first-run link, which signs in `account` (or makes an account when there is
    /// none given) when it is used. It is made once: `None` says one was made before, whether or
    /// not it was used, or that an account already has a way to sign in. The secret is returned
    /// here and nowhere else; the row holds its digest.
    pub async fn admin(
        &self,
        account: Option<Id<AccountKind>>,
        drawn: Drawn,
        now: Timestamp,
    ) -> Result<Option<Issued>, Error<D::Error>> {
        if self.found(table::Factor::Table, Expr::col(table::Factor::Id).is_not_null()).await? {
            return Ok(None);
        }
        let draft = Draft {
            purpose: Purpose::Admin,
            address: None,
            account,
            slot: Some(Self::SLOT),
            proof: drawn.proof,
            expires: self.settings.admin.after(now)?,
        };
        match self.issue(draft, now).await {
            Ok(issued) => Ok(Some(issued)),
            Err(Error::Db(error)) if error.constraint() == Some(Constraint::Unique) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The link a digest names, if it has not been used or ended. The stored digest is compared
    /// with the presented one in constant time, though the lookup already named it.
    async fn live(&self, digest: &Digest, now: Timestamp) -> Result<Option<(Link, Pending)>, Error<D::Error>> {
        let select = Query::select()
            .columns([
                table::Challenge::Digest,
                table::Challenge::Purpose,
                table::Challenge::Address,
                table::Challenge::AccountId,
                table::Challenge::ExpiresAt,
            ])
            .from(table::Challenge::Table)
            .and_where(Expr::col(table::Challenge::Digest).eq(digest.as_bytes().as_slice()))
            .and_where(Expr::col(table::Challenge::UsedAt).is_null())
            .and_where(Expr::col(table::Challenge::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        let Some(row) = self.db.first(select.into()).await? else { return Ok(None) };
        let fields = Fields(&row);
        if fields.digest(table::Challenge::Digest)? != *digest {
            return Ok(None);
        }
        let purpose = fields.name(table::Challenge::Purpose, &Purpose::NAMES)?;
        let expires = fields.time(table::Challenge::ExpiresAt)?;
        let link = Link {
            purpose,
            address: fields.maybe_address(table::Challenge::Address)?,
            account: fields.maybe_id(table::Challenge::AccountId)?,
        };
        Ok(Some((link, Pending { purpose, expires })))
    }

    /// What a link is for and when it stops working, without using it. `None` for a link that
    /// was never made, was used, or ended, which look the same.
    pub async fn peek(&self, proof: &Proof, now: Timestamp) -> Result<Option<Pending>, Error<D::Error>> {
        Ok(self.live(&Context::Challenge.digest(proof), now).await?.map(|(_, pending)| pending))
    }

    /// Uses a link and signs in. For the first-run link, `address` is the one to give the
    /// owner's account so it can sign in by mail later; for other links it is ignored.
    ///
    /// Everything that can be checked is checked before the link is spent. The link is spent
    /// by a conditional update that only one caller wins.
    pub async fn redeem(
        &self,
        presented: Presented<'_>,
        drawn: Drawn,
        now: Timestamp,
    ) -> Result<Entered, Error<D::Error>> {
        let Presented { proof, address } = presented;
        let digest = Context::Challenge.digest(proof);
        let (link, _) = self.live(&digest, now).await?.ok_or(Refused::Unusable)?;
        let (attach, fresh) = self.target(&link, address).await?;
        let account = link.account.unwrap_or(drawn.account);
        let spend = Query::update()
            .table(table::Challenge::Table)
            .value(table::Challenge::UsedAt, now.as_millisecond())
            .and_where(Expr::col(table::Challenge::Digest).eq(digest.as_bytes().as_slice()))
            .and_where(Expr::col(table::Challenge::UsedAt).is_null())
            .and_where(Expr::col(table::Challenge::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        if self.db.run(spend.into()).await?.changes != 1 {
            return Err(Refused::Unusable.into());
        }
        let ends = self.settings.session.begin(now)?;
        let session =
            Session { id: drawn.session, account, created: now, seen: now, expires: ends.expires, limit: ends.limit };
        let digest = Context::Session.digest(&drawn.proof);
        let making = Making { session, digest, account, fresh, attach, factor: drawn.factor, now };
        self.db.batch(Self::statements(&making)?).await?;
        Ok(Entered { session, secret: drawn.proof, account, created: fresh })
    }

    /// Signs in the account a passkey just proved, with a session of its own. The session is
    /// linked to the passkey, so removing the passkey ends it, and the sign-in is an event.
    /// Everything else about the session is the same as a link's: a 256-bit secret of which only
    /// the digest is stored, an absolute limit and an idle limit.
    pub async fn enter(&self, passed: Passed, drawn: Drawn, now: Timestamp) -> Result<Entered, Error<D::Error>> {
        let Passed { account, passkey } = passed;
        let ends = self.settings.session.begin(now)?;
        let session =
            Session { id: drawn.session, account, created: now, seen: now, expires: ends.expires, limit: ends.limit };
        let digest = Context::Session.digest(&drawn.proof);
        let link = Query::insert()
            .into_table(table::PasskeySession::Table)
            .columns([table::PasskeySession::SessionId, table::PasskeySession::PasskeyId])
            .values([Expr::val(session.id.to_string()), Expr::val(passkey.to_string())])?
            .to_owned();
        let audit = Audit::SignedIn { passkey }.statement(account, now)?;
        self.db.batch(vec![Self::row(&session, &digest)?, link.into(), audit]).await?;
        Ok(Entered { session, secret: drawn.proof, account, created: false })
    }

    /// Who a link signs in as: the address to give a new account, and whether the account is new.
    /// Every refusal that can be known before the link is spent is made here.
    async fn target(&self, link: &Link, address: Option<&Address>) -> Result<(Option<Address>, bool), Error<D::Error>> {
        let fresh = link.account.is_none();
        match link.purpose {
            Purpose::Login => {
                if fresh && !self.allowed().await? {
                    return Err(Refused::Closed.into());
                }
                Ok((link.address.clone().filter(|_| fresh), fresh))
            }
            Purpose::Admin => {
                if let Some(wanted) = address
                    && self.holder(wanted).await?.is_some()
                {
                    return Err(Refused::Taken { address: wanted.clone() }.into());
                }
                Ok((address.cloned(), fresh))
            }
        }
    }

    /// The rows of a sign-in, in the order they are written: the account, its way to sign in, the session.
    fn statements(making: &Making) -> Result<Vec<Statement>, Error<D::Error>> {
        let (account, now) = (making.account, making.now);
        let mut rows: Vec<Statement> = Vec::new();
        if making.fresh {
            rows.push(
                Query::insert()
                    .into_table(table::Account::Table)
                    .columns([table::Account::Id, table::Account::CreatedAt])
                    .values([Expr::val(account.to_string()), Expr::val(now.as_millisecond())])?
                    .to_owned()
                    .into(),
            );
        }
        if let Some(address) = &making.attach {
            rows.push(
                Query::insert()
                    .into_table(table::Factor::Table)
                    .columns([
                        table::Factor::Id,
                        table::Factor::AccountId,
                        table::Factor::Kind,
                        table::Factor::Address,
                        table::Factor::CreatedAt,
                    ])
                    .values([
                        Expr::val(making.factor.to_string()),
                        Expr::val(account.to_string()),
                        Expr::val(Sort::Link.name()),
                        Expr::val(address.as_str().to_owned()),
                        Expr::val(now.as_millisecond()),
                    ])?
                    .to_owned()
                    .into(),
            );
        }
        rows.push(Self::row(&making.session, &making.digest)?);
        Ok(rows)
    }

    fn row(session: &Session, digest: &Digest) -> Result<Statement, Error<D::Error>> {
        Ok(Query::insert()
            .into_table(table::Session::Table)
            .columns([
                table::Session::Id,
                table::Session::Digest,
                table::Session::AccountId,
                table::Session::CreatedAt,
                table::Session::SeenAt,
                table::Session::ExpiresAt,
                table::Session::LimitAt,
            ])
            .values([
                Expr::val(session.id.to_string()),
                Expr::val(digest.as_bytes().as_slice()),
                Expr::val(session.account.to_string()),
                Expr::val(session.created.as_millisecond()),
                Expr::val(session.seen.as_millisecond()),
                Expr::val(session.expires.as_millisecond()),
                Expr::val(session.limit.as_millisecond()),
            ])?
            .to_owned()
            .into())
    }

    /// The session a secret belongs to, if it was not revoked and has not ended. A sliding
    /// session is moved on by the use, as far as its limit.
    pub async fn resolve(&self, proof: &Proof, now: Timestamp) -> Result<Option<Session>, Error<D::Error>> {
        let digest = Context::Session.digest(proof);
        let select = Query::select()
            .columns(Session::COLUMNS)
            .from(table::Session::Table)
            .and_where(Expr::col(table::Session::Digest).eq(digest.as_bytes().as_slice()))
            .and_where(Expr::col(table::Session::RevokedAt).is_null())
            .and_where(Expr::col(table::Session::ExpiresAt).gt(now.as_millisecond()))
            .to_owned();
        let Some(row) = self.db.first(select.into()).await? else { return Ok(None) };
        if Fields(&row).digest(table::Session::Digest)? != digest {
            return Ok(None);
        }
        let mut session = Session::try_from(&row)?;
        if let Some(next) = self.settings.session.slide(now, session.ends())? {
            let touch = Query::update()
                .table(table::Session::Table)
                .value(table::Session::ExpiresAt, next.as_millisecond())
                .value(table::Session::SeenAt, now.as_millisecond())
                .and_where(Expr::col(table::Session::Id).eq(session.id.to_string()))
                .and_where(Expr::col(table::Session::RevokedAt).is_null())
                .to_owned();
            self.db.run(touch.into()).await?;
            session.expires = next;
            session.seen = now;
        }
        Ok(Some(session))
    }

    async fn revoked(&self, filter: Expr, now: Timestamp) -> Result<u64, Error<D::Error>> {
        let revoke = Query::update()
            .table(table::Session::Table)
            .value(table::Session::RevokedAt, now.as_millisecond())
            .and_where(Expr::col(table::Session::RevokedAt).is_null())
            .and_where(Expr::col(table::Session::ExpiresAt).gt(now.as_millisecond()))
            .and_where(filter)
            .to_owned();
        Ok(self.db.run(revoke.into()).await?.changes)
    }

    /// Ends the session a secret belongs to. Whether there was one to end.
    pub async fn end(&self, proof: &Proof, now: Timestamp) -> Result<bool, Error<D::Error>> {
        let digest = Context::Session.digest(proof);
        let filter = Expr::col(table::Session::Digest).eq(digest.as_bytes().as_slice());
        Ok(self.revoked(filter, now).await? == 1)
    }

    /// Ends one of an account's sessions, by its id. Another account's session is not found.
    pub async fn revoke(
        &self,
        account: Id<AccountKind>,
        id: Id<SessionKind>,
        now: Timestamp,
    ) -> Result<bool, Error<D::Error>> {
        let filter = Expr::col(table::Session::Id)
            .eq(id.to_string())
            .and(Expr::col(table::Session::AccountId).eq(account.to_string()));
        Ok(self.revoked(filter, now).await? == 1)
    }

    /// Ends every session of an account. How many there were.
    pub async fn revoke_all(&self, account: Id<AccountKind>, now: Timestamp) -> Result<u64, Error<D::Error>> {
        self.revoked(Expr::col(table::Session::AccountId).eq(account.to_string()), now).await
    }

    /// An account's sessions that are still good, newest first.
    pub async fn sessions(&self, account: Id<AccountKind>, now: Timestamp) -> Result<Vec<Session>, Error<D::Error>> {
        let select = Query::select()
            .columns(Session::COLUMNS)
            .from(table::Session::Table)
            .and_where(Expr::col(table::Session::AccountId).eq(account.to_string()))
            .and_where(Expr::col(table::Session::RevokedAt).is_null())
            .and_where(Expr::col(table::Session::ExpiresAt).gt(now.as_millisecond()))
            .order_by(table::Session::CreatedAt, Order::Desc)
            .order_by(table::Session::Id, Order::Asc)
            .to_owned();
        let rows = self.db.rows(select.into()).await?;
        Ok(rows.iter().map(Session::try_from).collect::<Result<_, _>>()?)
    }

    /// What an account can sign in with.
    pub async fn factors(&self, account: Id<AccountKind>) -> Result<Vec<Factor>, Error<D::Error>> {
        let select = Query::select()
            .columns([
                table::Factor::Id,
                table::Factor::AccountId,
                table::Factor::Kind,
                table::Factor::Address,
                table::Factor::CreatedAt,
            ])
            .from(table::Factor::Table)
            .and_where(Expr::col(table::Factor::AccountId).eq(account.to_string()))
            .order_by(table::Factor::CreatedAt, Order::Asc)
            .order_by(table::Factor::Id, Order::Asc)
            .to_owned();
        let rows = self.db.rows(select.into()).await?;
        Ok(rows.iter().map(Factor::try_from).collect::<Result<_, _>>()?)
    }

    /// Deletes links and sessions that have ended. The first-run link's row stays, so it cannot
    /// be made again.
    pub async fn purge(&self, now: Timestamp) -> Result<(), Error<D::Error>> {
        let links = Query::delete()
            .from_table(table::Challenge::Table)
            .and_where(Expr::col(table::Challenge::ExpiresAt).lt(now.as_millisecond()))
            .and_where(Expr::col(table::Challenge::Slot).is_null())
            .to_owned();
        let sessions = Query::delete()
            .from_table(table::Session::Table)
            .and_where(Expr::col(table::Session::ExpiresAt).lt(now.as_millisecond()))
            .to_owned();
        self.db.batch(vec![links.into(), sessions.into()]).await?;
        Ok(())
    }
}
