//! The adapter.

use std::time::Duration;

use db::{Db, Row, Statement};
use model::Timestamp;
use queue::{Delivery, Message, MessageId, Queue, Sent, Settled};
use sea_query::{Expr, ExprTrait, OnConflict, Order, Query};
use logging::Field as _;

use crate::Error;
use crate::schema::Message as Table;

type Failure<D> = queue::Error<Error<<D as Db>::Error>>;

/// A queue in the rows of a database.
#[derive(Debug)]
pub struct Local<D> {
    db: D,
}

impl<D: Db> Local<D> {
    /// A queue in `db`, whose table the [`Queued`](crate::Queued) migration made.
    pub const fn new(db: D) -> Self {
        Self { db }
    }

    /// The database, to run it in the same one as other tables.
    pub const fn db(&self) -> &D {
        &self.db
    }

    fn later(now: Timestamp, span: Duration) -> Result<i64, Error<D::Error>> {
        now.checked_add(span).map(Timestamp::as_millisecond).map_err(|source| Error::Time { source })
    }

    fn backend(error: Error<D::Error>) -> Failure<D> {
        queue::Error::Backend { source: error }
    }

    async fn run(&self, statement: Statement) -> Result<db::Outcome, Failure<D>> {
        self.db.run(statement).await.map_err(|error| Self::backend(error.into()))
    }

    fn id(row: &Row) -> Result<MessageId, Failure<D>> {
        row.get::<u64>(Table::Id).map(MessageId).map_err(|error| Self::backend(error.into()))
    }

    fn settled(outcome: &db::Outcome) -> Settled {
        if outcome.changes == 1 { Settled::Done } else { Settled::Stale }
    }

    /// Says how settling `delivery` went, as `how`. A stale settlement changed nothing because
    /// the lease had ended: a `warn`, since the work it was for may run twice.
    fn said(how: &'static str, delivery: &Delivery, settled: Settled) -> Settled {
        let id = logging::Count(delivery.id.0).field();
        if settled == Settled::Done {
            logging::debug!(name: logging::event::QUEUE_SETTLED, how, id, outcome = "done");
        } else {
            logging::warn!(name: logging::event::QUEUE_SETTLED, how, id, outcome = "stale");
        }
        settled
    }
}

impl<D: Db> Queue for Local<D> {
    type Error = Error<D::Error>;

    async fn send(&self, message: Message, now: Timestamp) -> Result<Sent, Failure<D>> {
        let key = message.dedupe.as_ref().map(|dedupe| dedupe.as_str().to_owned());
        loop {
            let insert = Query::insert()
                .into_table(Table::Table)
                .columns([Table::Body, Table::Dedupe, Table::VisibleAt])
                .values([Expr::val(message.body.clone()), Expr::val(key.clone()), Expr::val(now.as_millisecond())])
                .map_err(|source| Self::backend(Error::Statement { source }))?
                .on_conflict(OnConflict::column(Table::Dedupe).do_nothing().to_owned())
                .returning_col(Table::Id)
                .to_owned();
            if let Some(row) = self.run(insert.into()).await?.rows.first() {
                let id = Self::id(row)?;
                logging::debug!(name: logging::event::QUEUE_SENT, id = logging::Count(id.0).field(), outcome = "queued");
                return Ok(Sent::Queued(id));
            }
            // not inserted, so the key is held; unless the holder was acked in between, then try again
            let held = Query::select()
                .column(Table::Id)
                .from(Table::Table)
                .and_where(Expr::col(Table::Dedupe).eq(key.clone()))
                .to_owned();
            if let Some(row) = self.run(held.into()).await?.rows.first() {
                let id = Self::id(row)?;
                logging::debug!(
                    name: logging::event::QUEUE_SENT,
                    id = logging::Count(id.0).field(),
                    outcome = "duplicate",
                );
                return Ok(Sent::Duplicate(id));
            }
        }
    }

    async fn receive(&self, now: Timestamp, lease: Duration) -> Result<Option<Delivery>, Failure<D>> {
        let until = Self::later(now, lease).map_err(Self::backend)?;
        let next = Query::select()
            .column(Table::Id)
            .from(Table::Table)
            .and_where(Expr::col(Table::VisibleAt).lte(now.as_millisecond()))
            .order_by(Table::VisibleAt, Order::Asc)
            .order_by(Table::Id, Order::Asc)
            .limit(1)
            .to_owned();
        let claim = Query::update()
            .table(Table::Table)
            .value(Table::VisibleAt, until)
            .value(Table::Attempt, Expr::col(Table::Attempt).add(1))
            .and_where(Expr::col(Table::Id).in_subquery(next))
            .returning(Query::returning().columns([Table::Id, Table::Body, Table::Attempt]))
            .to_owned();
        let claimed = self.run(claim.into()).await?;
        let Some(row) = claimed.rows.first() else { return Ok(None) };
        let read = |row: &Row| -> Result<Delivery, db::ColumnError> {
            Ok(Delivery {
                id: MessageId(row.get(Table::Id)?),
                attempt: row.get(Table::Attempt)?,
                body: row.get(Table::Body)?,
            })
        };
        let delivery = read(row).map_err(|error| Self::backend(error.into()))?;
        logging::debug!(
            name: logging::event::QUEUE_RECEIVED,
            id = logging::Count(delivery.id.0).field(),
            attempt = logging::Count(u64::from(delivery.attempt)).field(),
        );
        Ok(Some(delivery))
    }

    async fn ack(&self, delivery: &Delivery) -> Result<Settled, Failure<D>> {
        let delete = Query::delete()
            .from_table(Table::Table)
            .and_where(Expr::col(Table::Id).eq(delivery.id.0.cast_signed()))
            .and_where(Expr::col(Table::Attempt).eq(delivery.attempt))
            .to_owned();
        Ok(Self::said("ack", delivery, Self::settled(&self.run(delete.into()).await?)))
    }

    async fn release(&self, delivery: &Delivery, now: Timestamp, delay: Duration) -> Result<Settled, Failure<D>> {
        let at = Self::later(now, delay).map_err(Self::backend)?;
        let release = Query::update()
            .table(Table::Table)
            .value(Table::VisibleAt, at)
            .and_where(Expr::col(Table::Id).eq(delivery.id.0.cast_signed()))
            .and_where(Expr::col(Table::Attempt).eq(delivery.attempt))
            .to_owned();
        Ok(Self::said("release", delivery, Self::settled(&self.run(release.into()).await?)))
    }
}
