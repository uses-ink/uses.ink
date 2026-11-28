//! The recovery code store, over the row port alone.

use db::{Db, Statement};
use jiff::Timestamp;
use sea_query::{Expr, ExprTrait, Query};
use serde_json::json;

use super::{Code, Error};
use crate::fields::Fields;
use crate::id::{Account as AccountKind, Id};
use crate::signin::Digest;
use crate::table;

/// What minting a set draws: the bytes of each code and the id of each row. The caller draws them
/// from its own source of randomness.
#[derive(Debug)]
pub struct Mint {
    /// The 128 bits of each code.
    pub codes: [[u8; 16]; Mint::COUNT],
    /// The id of each code's row, which names it in the audit log and is no use to recover with.
    pub ids: [[u8; 16]; Mint::COUNT],
}

impl Mint {
    /// How many codes a set has.
    pub const COUNT: usize = 10;
}

/// One event of the audit log that concerns recovery codes. None names a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Audit {
    /// A new set was made, and the old one ended.
    Minted,
    /// A code was spent to register a passkey.
    Used {
        /// The code's row id.
        code: Id<crate::id::Factor>,
    },
}

impl Audit {
    /// The name the log stores.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Minted => "recovery.minted",
            Self::Used { .. } => "recovery.used",
        }
    }

    fn statement(self, account: Id<AccountKind>, now: Timestamp) -> Result<Statement, sea_query::error::Error> {
        let (key, body) = match self {
            Self::Minted => (account.to_string(), json!({"event": "minted"})),
            Self::Used { code } => (code.to_string(), json!({"event": "used", "code": code.to_string()})),
        };
        Ok(Query::insert()
            .into_table(table::AuthEvent::Table)
            .columns([
                table::AuthEvent::AccountId,
                table::AuthEvent::KeyId,
                table::AuthEvent::At,
                table::AuthEvent::Name,
                table::AuthEvent::Body,
            ])
            .values([
                Expr::val(account.to_string()),
                Expr::val(key),
                Expr::val(now.as_millisecond()),
                Expr::val(self.name()),
                Expr::val(body.to_string()),
            ])?
            .to_owned()
            .into())
    }
}

/// The recovery codes of accounts, over a database. Only digests are stored; a code is shown once,
/// when it is minted, and is spent by one conditional update that only one caller wins.
#[derive(Clone, Debug)]
pub struct Codes<D> {
    db: D,
}

impl<D: Db> Codes<D> {
    /// A store over `db`.
    pub const fn new(db: D) -> Self {
        Self { db }
    }

    /// Makes a set of codes for `account` and ends the old one, in one batch, so there is never a
    /// moment with two sets or none. The codes are returned here and kept nowhere.
    pub async fn mint(&self, account: Id<AccountKind>, drawn: Mint, now: Timestamp) -> Result<Vec<Code>, Error<D::Error>> {
        let mut batch: Vec<Statement> = vec![
            Query::delete()
                .from_table(table::RecoveryCode::Table)
                .and_where(Expr::col(table::RecoveryCode::AccountId).eq(account.to_string()))
                .to_owned()
                .into(),
        ];
        let mut codes = Vec::with_capacity(Mint::COUNT);
        for (bytes, id) in drawn.codes.into_iter().zip(drawn.ids) {
            let code = Code::from_bytes(bytes);
            let row: Id<crate::id::Factor> = Id::from_bytes(id);
            batch.push(
                Query::insert()
                    .into_table(table::RecoveryCode::Table)
                    .columns([
                        table::RecoveryCode::Id,
                        table::RecoveryCode::Digest,
                        table::RecoveryCode::AccountId,
                        table::RecoveryCode::CreatedAt,
                    ])
                    .values([
                        Expr::val(row.to_string()),
                        Expr::val(code.digest().as_bytes().as_slice()),
                        Expr::val(account.to_string()),
                        Expr::val(now.as_millisecond()),
                    ])?
                    .to_owned()
                    .into(),
            );
            codes.push(code);
        }
        batch.push(Audit::Minted.statement(account, now)?);
        self.db.batch(batch).await?;
        Ok(codes)
    }

    /// How many of `account`'s codes are unspent.
    pub async fn left(&self, account: Id<AccountKind>) -> Result<u64, Error<D::Error>> {
        let select = Query::select()
            .expr_as(Expr::col(table::RecoveryCode::Id).count(), table::Computed::Count)
            .from(table::RecoveryCode::Table)
            .and_where(Expr::col(table::RecoveryCode::AccountId).eq(account.to_string()))
            .and_where(Expr::col(table::RecoveryCode::UsedAt).is_null())
            .to_owned();
        let row = self.db.first(select.into()).await?;
        Ok(row.map(|row| Fields(&row).count(table::Computed::Count)).transpose()?.unwrap_or(0))
    }

    /// Spends `code`: the account it belongs to, or `None` when no unspent code has this digest,
    /// whether it was never made, was spent or was replaced by a new set (the three look the same).
    /// The stored digest is compared in constant time, though the lookup already named the row.
    pub async fn spend(&self, code: &Code, now: Timestamp) -> Result<Option<Id<AccountKind>>, Error<D::Error>> {
        let digest = code.digest();
        let select = Query::select()
            .columns([table::RecoveryCode::Id, table::RecoveryCode::Digest, table::RecoveryCode::AccountId])
            .from(table::RecoveryCode::Table)
            .and_where(Expr::col(table::RecoveryCode::Digest).eq(digest.as_bytes().as_slice()))
            .and_where(Expr::col(table::RecoveryCode::UsedAt).is_null())
            .to_owned();
        let Some(row) = self.db.first(select.into()).await? else { return Ok(None) };
        let fields = Fields(&row);
        let stored: Digest = fields.digest(table::RecoveryCode::Digest)?;
        if stored != digest {
            return Ok(None);
        }
        let (id, account): (Id<crate::id::Factor>, Id<AccountKind>) =
            (fields.id(table::RecoveryCode::Id)?, fields.id(table::RecoveryCode::AccountId)?);
        let spend = Query::update()
            .table(table::RecoveryCode::Table)
            .value(table::RecoveryCode::UsedAt, now.as_millisecond())
            .and_where(Expr::col(table::RecoveryCode::Id).eq(id.to_string()))
            .and_where(Expr::col(table::RecoveryCode::UsedAt).is_null())
            .to_owned();
        if self.db.run(spend.into()).await?.changes != 1 {
            return Ok(None);
        }
        self.db.run(Audit::Used { code: id }.statement(account, now)?).await?;
        Ok(Some(account))
    }
}
