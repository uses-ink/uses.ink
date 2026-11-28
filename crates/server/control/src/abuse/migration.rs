//! The abuse tables, as a migration after the hold's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, Table};

use super::{Abuse, ReportState};
use crate::table::{Allowed, Report, Suspension};
use crate::typed::Typed;

/// The migration that makes `abuse_report` and `suspension`. A suspension is a row of its own and
/// not a state of the site, so the site's rows, its checks and every migration before this one
/// stay as they are, and reinstating a site is deleting one row.
#[derive(Clone, Copy, Debug)]
pub struct Reported;

impl Migration for Reported {
    const NUMBER: u32 = 303;
    const NAME: &'static str = "abuse";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Report::Table)
                .col(ColumnDef::new(Report::Id).text().not_null().primary_key())
                .col(Typed::text(Report::Handle))
                .col(ColumnDef::new(Report::Kind).text().not_null().check(Allowed::of(Report::Kind, &Abuse::NAMES)))
                .col(Typed::text(Report::Detail))
                .col(ColumnDef::new(Report::Contact).text())
                .col(
                    ColumnDef::new(Report::State)
                        .text()
                        .not_null()
                        .check(Allowed::of(Report::State, &ReportState::NAMES)),
                )
                .col(Typed::time(Report::At))
                .into(),
            Table::create()
                .table(Suspension::Table)
                .col(ColumnDef::new(Suspension::SiteId).text().not_null().primary_key())
                .col(
                    ColumnDef::new(Suspension::Kind)
                        .text()
                        .not_null()
                        .check(Allowed::of(Suspension::Kind, &Abuse::NAMES)),
                )
                .col(Typed::text(Suspension::By))
                .col(ColumnDef::new(Suspension::ReportId).text())
                .col(Typed::time(Suspension::At))
                .into(),
        ]
    }
}
