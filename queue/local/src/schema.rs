//! The queue's table, as typed identifiers, and the migration that makes it.

use db::{Migration, Statement};
use sea_query::{ColumnDef, Expr, IdenStatic, Index, Table};

/// The queue's table and its columns.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "queue_message"]
pub(crate) enum Message {
    Table,
    Id,
    Body,
    Dedupe,
    VisibleAt,
    Attempt,
}

/// The index the claim reads: the next visible message.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "queue_message_visible"]
struct Visible;

/// The migration that makes the queue's table. At-least-once: a message is hidden while leased
/// (`visible_at` in the future) and delivered again when the lease ends.
#[derive(Clone, Copy, Debug)]
pub struct Queued;

impl Migration for Queued {
    const NUMBER: u32 = 100;
    const NAME: &'static str = "queue";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Message::Table)
                .col(ColumnDef::new(Message::Id).integer().not_null().auto_increment().primary_key())
                .col(ColumnDef::new(Message::Body).blob().not_null())
                // a key makes a message once while it is in the queue; keyless messages are never deduped
                .col(ColumnDef::new(Message::Dedupe).text().unique_key())
                // unix milliseconds
                .col(ColumnDef::new(Message::VisibleAt).big_integer().not_null())
                .col(ColumnDef::new(Message::Attempt).integer().not_null().default(Expr::value(0_i64)))
                .into(),
            Index::create().name(Visible.as_str()).table(Message::Table).col(Message::VisibleAt).col(Message::Id).into(),
        ]
    }
}
