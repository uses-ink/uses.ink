//! `ink abuse list|suspend|reinstate`: the operator's side of reports and suspensions (decision
//! 0089), over the API's operator routes. Every call is signed with the operator's key (`ink
//! login --data`); a key that is not the operator's is refused by the server.

use clap::{Args, Subcommand};
use control::{AbuseKind, Id, ReportKind, SiteKind};
use edge_api::view::Suspended;
use hyper::Method;
use jiff::Timestamp;
use serde::Deserialize;
use serde::Serialize;

use crate::command::auth::Server;
use crate::deploy::{Failure, Moment};
use crate::output::Io;
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::term::{Doc, Empty, Line, Listed, Record, Render, Role};
use crate::settings::{Preferences, Process};

/// `ink abuse <list|suspend|reinstate>`: a group with no behaviour of its own.
#[derive(Debug, Args)]
pub struct Abuse {
    /// What to do about reports.
    #[command(subcommand)]
    pub command: Reports,
}

/// What `ink abuse` does.
#[derive(Debug, Subcommand)]
pub enum Reports {
    /// The reports of strangers, newest first.
    List(Server),
    /// Take a site down.
    Suspend(Suspend),
    /// Lift a suspension.
    Reinstate(Reinstate),
}

/// `ink abuse suspend <SITE>`.
#[derive(Debug, Args)]
pub struct Suspend {
    /// The site's id, as `ink abuse list` or the site's owner gives it.
    pub site: Id<SiteKind>,
    /// Why: phishing, malware, illegal, impersonation, spam or other.
    #[arg(long, value_name = "KIND", default_value = "other")]
    pub kind: AbuseKind,
    /// The report that led to it, as `ink abuse list` shows its id.
    #[arg(long, value_name = "ID")]
    pub report: Option<Id<ReportKind>>,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `ink abuse reinstate <SITE>`.
#[derive(Debug, Args)]
pub struct Reinstate {
    /// The site's id.
    pub site: Id<SiteKind>,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// A report, as `GET /v1/abuse` lists it.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Row {
    id: Id<ReportKind>,
    handle: String,
    kind: AbuseKind,
    detail: String,
    contact: Option<String>,
    state: String,
    at: Timestamp,
}

/// `GET /v1/abuse`: `{items: [...]}`.
#[derive(Debug, Deserialize)]
struct Items {
    items: Vec<Row>,
}

/// What `ink abuse list --json` prints.
#[derive(Debug, Serialize)]
struct ReportsOut<'a> {
    schema: &'static str,
    reports: &'a [Row],
}

/// What `ink abuse suspend` and `reinstate` print with `--json`.
#[derive(Debug, Serialize)]
struct SuspendedOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    suspended: &'a Suspended,
}

/// The body of `POST /v1/sites/{site}/suspend`.
#[derive(Debug, Serialize)]
struct Taking {
    kind: AbuseKind,
    report: Option<Id<ReportKind>>,
}

/// What the list of reports says when no stranger sent one.
const EMPTY: Empty = Empty { said: "No reports.", next: None };

/// The reports as a table.
#[derive(Debug)]
struct Listing<'a>(&'a [Row]);

impl Render for Listing<'_> {
    fn render(&self, into: &mut Doc) {
        let mut table = Listed::new(EMPTY, &["id", "handle", "kind", "state", "came", "contact"]);
        for report in self.0 {
            let state = if report.state == "open" { Role::Warn } else { Role::Good };
            table = table.row(vec![
                Line::new().with(Role::Id, report.id.to_string()),
                Line::plain(report.handle.clone()),
                Line::plain(report.kind.to_string()),
                Line::new().with(state, report.state.clone()),
                Line::plain(Moment(report.at).to_string()),
                Line::plain(report.contact.clone().unwrap_or_else(|| "none".to_owned())),
            ]);
            if !report.detail.is_empty() {
                table =
                    table.row(vec![Line::plain(String::new()), Line::new().with(Role::Quiet, report.detail.clone())]);
            }
        }
        table.render(into);
    }
}

/// A site taken down or brought back.
#[derive(Debug)]
struct Changed<'a>(&'a Suspended);

impl Render for Changed<'_> {
    fn render(&self, into: &mut Doc) {
        let (role, word) = match self.0.kind {
            Some(_) => (Role::Warn, "suspended"),
            None => (Role::Good, "reinstated"),
        };
        let mut record = Record::new(0)
            .field("site", Line::new().with(Role::Id, self.0.site.to_string()))
            .field("state", Line::new().with(role, word));
        if let Some(kind) = self.0.kind {
            record = record.field("kind", Line::plain(kind.to_string()));
        }
        record.field("at", Line::plain(Moment(self.0.at).to_string())).render(into);
    }
}

impl Reports {
    /// Runs the command.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        match self {
            Self::List(server) => {
                let calls = server.signed(&Process, &preferences)?;
                let items: Items = calls.json(Method::GET, "/v1/abuse", Vec::new())?;
                io.result(&Listing(&items.items))?;
                io.json(&ReportsOut { schema: "uses.abuse/v1", reports: &items.items })?;
            }
            Self::Suspend(suspend) => {
                let calls = suspend.server.signed(&Process, &preferences)?;
                let body = serde_json::to_vec(&Taking { kind: suspend.kind, report: suspend.report })
                    .map_err(|source| Failure::Unreadable { source })?;
                let made: Suspended = calls.json(Method::POST, &format!("/v1/sites/{}/suspend", suspend.site), body)?;
                io.result(&Changed(&made))?;
                io.json(&SuspendedOut { schema: "uses.suspension/v1", suspended: &made })?;
            }
            Self::Reinstate(reinstate) => {
                let calls = reinstate.server.signed(&Process, &preferences)?;
                let path = format!("/v1/sites/{}/reinstate", reinstate.site);
                let made: Suspended = calls.json(Method::POST, &path, b"{}".to_vec())?;
                io.result(&Changed(&made))?;
                io.json(&SuspendedOut { schema: "uses.suspension/v1", suspended: &made })?;
            }
        }
        Ok(Exit::Done)
    }
}
