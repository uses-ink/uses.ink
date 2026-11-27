//! The adapter.

use std::time::Duration;

use db::{Db, Row, Statement, Step};
use model::Timestamp;
use queue::{Delivery, Message, MessageId, Queue, Sent, Settled};

use crate::Error;

type Failure<D> = queue::Error<Error<<D as Db>::Error>>;

/// A queue in the rows of a database.
#[derive(Debug)]
pub struct Local<D> {
    db: D,
}

impl<D: Db> Local<D> {
    /// The migrations that make the queue's table: apply them with the host's other steps.
    pub const STEPS: [Step; 1] = [Step::new("0100_queue.sql", include_str!("../../../migration/0100_queue.sql"))];

    /// A queue in `db`, whose table [`Local::STEPS`] made.
    pub const fn new(db: D) -> Self {
        Self { db }
    }

    /// The database, to run it in the same one as other tables.
    pub const fn db(&self) -> &D {
        &self.db
    }

    fn later(now: Timestamp, span: Duration) -> Result<i64, Error<D::Error>> {
        now.checked_add(span).map(|at| at.as_millisecond()).map_err(|source| Error::Time { source })
    }

    fn backend(error: Error<D::Error>) -> Failure<D> {
        queue::Error::Backend { source: error }
    }

    async fn run(&self, statement: Statement) -> Result<db::Outcome, Failure<D>> {
        self.db.run(statement).await.map_err(|error| Self::backend(error.into()))
    }

    fn id(row: &Row) -> Result<MessageId, Failure<D>> {
        row.get::<u64>("id").map(MessageId).map_err(|error| Self::backend(error.into()))
    }

    fn settled(outcome: &db::Outcome) -> Settled {
        if outcome.changes == 1 { Settled::Done } else { Settled::Stale }
    }
}

impl<D: Db> Queue for Local<D> {
    type Error = Error<D::Error>;

    async fn send(&self, message: Message, now: Timestamp) -> Result<Sent, Failure<D>> {
        const INSERT: &str = "INSERT INTO queue_message (body, dedupe, visible_at) VALUES (?, ?, ?) \
            ON CONFLICT (dedupe) DO NOTHING RETURNING id";
        const HELD: &str = "SELECT id FROM queue_message WHERE dedupe = ?";
        let key = message.dedupe.as_ref().map(|dedupe| dedupe.as_str().to_owned());
        loop {
            let insert = Statement::prepare(INSERT).bind(message.body.clone()).bind(key.clone()).bind(now.as_millisecond());
            if let Some(row) = self.run(insert).await?.rows.first() {
                return Ok(Sent::Queued(Self::id(row)?));
            }
            // not inserted, so the key is held; unless the holder was acked in between, then try again
            if let Some(row) = self.run(Statement::prepare(HELD).bind(key.clone())).await?.rows.first() {
                return Ok(Sent::Duplicate(Self::id(row)?));
            }
        }
    }

    async fn receive(&self, now: Timestamp, lease: Duration) -> Result<Option<Delivery>, Failure<D>> {
        const CLAIM: &str = "UPDATE queue_message SET visible_at = ?, attempt = attempt + 1 \
            WHERE id = (SELECT id FROM queue_message WHERE visible_at <= ? ORDER BY visible_at, id LIMIT 1) \
            RETURNING id, body, attempt";
        let until = Self::later(now, lease).map_err(Self::backend)?;
        let claimed = self.run(Statement::prepare(CLAIM).bind(until).bind(now.as_millisecond())).await?;
        let Some(row) = claimed.rows.first() else { return Ok(None) };
        let read = |row: &Row| -> Result<Delivery, db::ColumnError> {
            Ok(Delivery { id: MessageId(row.get("id")?), attempt: row.get("attempt")?, body: row.get("body")? })
        };
        read(row).map(Some).map_err(|error| Self::backend(error.into()))
    }

    async fn ack(&self, delivery: &Delivery) -> Result<Settled, Failure<D>> {
        const DELETE: &str = "DELETE FROM queue_message WHERE id = ? AND attempt = ?";
        let outcome = self.run(Statement::prepare(DELETE).bind(delivery.id.0.cast_signed()).bind(delivery.attempt)).await?;
        Ok(Self::settled(&outcome))
    }

    async fn release(&self, delivery: &Delivery, now: Timestamp, delay: Duration) -> Result<Settled, Failure<D>> {
        const RELEASE: &str = "UPDATE queue_message SET visible_at = ? WHERE id = ? AND attempt = ?";
        let at = Self::later(now, delay).map_err(Self::backend)?;
        let outcome = self.run(Statement::prepare(RELEASE).bind(at).bind(delivery.id.0.cast_signed()).bind(delivery.attempt)).await?;
        Ok(Self::settled(&outcome))
    }
}
