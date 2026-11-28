//! `ink domain add|list|prove|remove`: a site's custom domains (decision 0093), over the API's
//! domain routes. `add` prints the TXT record to publish, `prove` has the server read it.

use clap::{Args, Subcommand};
use control::DomainName;
use edge_api::view::{DomainView, Unclaimed};
use serde::Serialize;

use crate::deploy::{Moment, Remote};
use crate::output::Io;
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::term::{Doc, Line, Record, Render, Role, Table};

/// `ink domain <add|list|prove|remove>`: a group with no behaviour of its own.
#[derive(Debug, Args)]
pub struct Domain {
    /// What to do with the domains.
    #[command(subcommand)]
    pub command: Names,
    /// The site's folder, from `-C`, whose link names the site.
    #[arg(skip)]
    pub dir: std::path::PathBuf,
}

/// What `ink domain` does.
#[derive(Debug, Subcommand)]
pub enum Names {
    /// Ask to prove a custom domain, and print the record to publish.
    Add(Named),
    /// The names that serve the site and the ones still to prove.
    List(Remote),
    /// Check the record and serve the site at the name.
    Prove(Named),
    /// Stop serving a name, or drop its claim.
    Remove(Named),
}

/// A command about one name.
#[derive(Debug, Args)]
pub struct Named {
    /// The name, such as `www.example.org`.
    pub name: DomainName,
    /// Which site.
    #[command(flatten)]
    pub remote: Remote,
}

/// What `ink domain add`, `prove` and `list --json` print: one domain.
#[derive(Debug, Serialize)]
struct DomainOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    domain: &'a DomainView,
}

/// What `ink domain list --json` prints.
#[derive(Debug, Serialize)]
struct DomainsOut<'a> {
    schema: &'static str,
    site: String,
    domains: &'a [DomainView],
}

/// What `ink domain remove --json` prints.
#[derive(Debug, Serialize)]
struct GoneOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    gone: &'a Unclaimed,
}

/// One domain as a record: where it stands and, while it is pending, what to publish.
#[derive(Debug)]
struct Standing<'a>(&'a DomainView);

impl Render for Standing<'_> {
    fn render(&self, into: &mut Doc) {
        let domain = self.0;
        let (role, word) =
            if domain.proven.is_some() { (Role::Good, "serving") } else { (Role::Warn, "waiting for proof") };
        let mut record = Record::new(0)
            .field("name", Line::new().with(Role::Id, domain.name.clone()))
            .field("state", Line::new().with(role, word));
        if let Some(at) = domain.proven {
            record = record.field("since", Line::plain(Moment(at).to_string()));
        }
        if let Some(publish) = &domain.record {
            record = record.field("publish", Line::plain(format!("{} TXT \"{}\"", publish.host, publish.value)));
        }
        record.render(into);
    }
}

/// The domains as a table.
#[derive(Debug)]
struct Listing<'a>(&'a [DomainView]);

impl Render for Listing<'_> {
    fn render(&self, into: &mut Doc) {
        if self.0.is_empty() {
            into.push(Line::plain("no custom domains"));
            return;
        }
        let quiet = |text: &str| Line::new().with(Role::Quiet, text);
        let mut table = Table::default().indent(2).row(["name", "state", "since or record"].map(quiet).to_vec());
        for domain in self.0 {
            let (role, word) =
                if domain.proven.is_some() { (Role::Good, "serving") } else { (Role::Warn, "waiting for proof") };
            let last = match (&domain.proven, &domain.record) {
                (Some(at), _) => Moment(*at).to_string(),
                (None, Some(record)) => format!("{} TXT \"{}\"", record.host, record.value),
                (None, None) => String::new(),
            };
            table = table.row(vec![
                Line::new().with(Role::Id, domain.name.clone()),
                Line::new().with(role, word),
                Line::plain(last),
            ]);
        }
        table.render(into);
    }
}

/// A name that stopped being served or claimed.
#[derive(Debug)]
struct Gone<'a>(&'a Unclaimed);

impl Render for Gone<'_> {
    fn render(&self, into: &mut Doc) {
        Record::new(0)
            .field("name", Line::new().with(Role::Id, self.0.name.clone()))
            .field("state", Line::new().with(Role::Good, "removed"))
            .render(into);
    }
}

impl Names {
    /// Runs the command.
    pub(crate) fn run(self, dir: &std::path::Path, io: &mut Io<'_>) -> Result<Exit, Error> {
        let fail = |source| Error::Deploy { source };
        match self {
            Self::Add(named) => {
                let calls = named.remote.folder(dir)?;
                let domain = calls.block(calls.api.claim(&named.name)).map_err(fail)?;
                io.result(&Standing(&domain))?;
                io.json(&DomainOut { schema: "uses.domain/v1", domain: &domain })?;
            }
            Self::Prove(named) => {
                let calls = named.remote.folder(dir)?;
                let domain = calls.block(calls.api.prove(&named.name)).map_err(fail)?;
                io.result(&Standing(&domain))?;
                io.json(&DomainOut { schema: "uses.domain/v1", domain: &domain })?;
            }
            Self::List(remote) => {
                let calls = remote.folder(dir)?;
                let domains = calls.block(calls.api.domains()).map_err(fail)?;
                io.result(&Listing(&domains))?;
                io.json(&DomainsOut {
                    schema: "uses.domains/v1",
                    site: calls.api.site.to_string(),
                    domains: &domains,
                })?;
            }
            Self::Remove(named) => {
                let calls = named.remote.folder(dir)?;
                let gone = calls.block(calls.api.unclaim(&named.name)).map_err(fail)?;
                io.result(&Gone(&gone))?;
                io.json(&GoneOut { schema: "uses.domain-removed/v1", gone: &gone })?;
            }
        }
        Ok(Exit::Done)
    }
}
