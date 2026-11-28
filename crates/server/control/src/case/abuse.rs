//! Abuse reports and suspension (decision 0089): the operator takes a site down first and decides
//! later whether to delete it.

use db::Db;
use jiff::Timestamp;
use sea_query::{Expr, ExprTrait, Order, Query};

use crate::id::{Id, Site as SiteKind};
use crate::{Control, Error, Event, NewReport, Ports, Refusal, Report, ReportState, Suspension, table};

impl<P: Ports> Control<P> {
    /// Records a report, open, and returns it. A report names a handle and not a site, so what it
    /// says is kept even when the site has gone, and nothing answers whether the handle is one.
    pub async fn report(&self, new: NewReport) -> Result<Report, Error<P>> {
        self.ports.db().batch(vec![Report::insert(&new)?]).await?;
        Ok(Report {
            id: new.id,
            handle: new.handle,
            kind: new.kind,
            detail: new.detail,
            contact: new.contact,
            state: ReportState::Open,
            at: new.at,
        })
    }

    /// The newest `limit` reports, newest first.
    pub async fn reports(&self, limit: u64) -> Result<Vec<Report>, Error<P>> {
        let select = Query::select()
            .columns(Report::COLUMNS)
            .from(table::Report::Table)
            .order_by(table::Report::At, Order::Desc)
            .order_by(table::Report::Id, Order::Desc)
            .limit(limit)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(Report::read).collect::<Result<_, _>>()?)
    }

    /// The suspension of `site`, if it has one.
    pub async fn suspension(&self, site: Id<SiteKind>) -> Result<Option<Suspension>, Error<P>> {
        let select = Query::select()
            .columns(Suspension::COLUMNS)
            .from(table::Suspension::Table)
            .and_where(Expr::col(table::Suspension::SiteId).eq(site.to_string()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Suspension::read(&row)).transpose()?)
    }

    /// Records `suspension` of its site: the site serves nothing and takes no deploys or builds
    /// until it is reinstated. Its files and rows are untouched, so lifting it gives back what
    /// was there. The report that led to it, if any, becomes actioned in the same step.
    pub async fn suspend(&self, suspension: Suspension) -> Result<Suspension, Error<P>> {
        let Suspension { site, kind, report, at, .. } = suspension;
        self.site(site).await?;
        if let Some(existing) = self.suspension(site).await? {
            return Err(Refusal::Suspended { site, kind: existing.kind }.into());
        }
        let mut batch = vec![suspension.insert()?, Event::SiteSuspended { kind }.insert(site, at)?];
        if let Some(report) = report {
            batch.push(
                Query::update()
                    .table(table::Report::Table)
                    .value(table::Report::State, ReportState::Actioned.name())
                    .and_where(Expr::col(table::Report::Id).eq(report.to_string()))
                    .to_owned()
                    .into(),
            );
        }
        self.ports.db().batch(batch).await?;
        Ok(suspension)
    }

    /// Lifts the suspension of `site`.
    pub async fn reinstate(&self, site: Id<SiteKind>, at: Timestamp) -> Result<(), Error<P>> {
        self.site(site).await?;
        if self.suspension(site).await?.is_none() {
            return Err(Refusal::Serving { site }.into());
        }
        let lift = Query::delete()
            .from_table(table::Suspension::Table)
            .and_where(Expr::col(table::Suspension::SiteId).eq(site.to_string()))
            .to_owned();
        self.ports.db().batch(vec![lift.into(), Event::SiteReinstated.insert(site, at)?]).await?;
        Ok(())
    }
}
