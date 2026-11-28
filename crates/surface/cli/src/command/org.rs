//! `ink org create|list|member`: organizations and who is in them (decision 0095), over the
//! API's org routes. A change of members needs the `members` right on the key, which only a key
//! approved with a passkey sign-in can have (`ink login --rights ..., members`).

use clap::{Args, Subcommand};
use control::{AccountKind, Id, OrgName, Role};
use edge_api::view::{MemberView, OrgMembers, OrgView, Unmembered};
use hyper::Method;
use serde::{Deserialize, Serialize};

use crate::command::auth::Server;
use crate::deploy::{Failure, Moment};
use crate::output::Io;
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::term::{Doc, Empty, Line, Listed, Record, Render, Role as Tone, Table};
use crate::settings::{Preferences, Process};

/// `ink org <create|list|member>`: a group with no behaviour of its own.
#[derive(Debug, Args)]
pub struct Org {
    /// What to do with organizations.
    #[command(subcommand)]
    pub command: Orgs,
}

/// What `ink org` does.
#[derive(Debug, Subcommand)]
pub enum Orgs {
    /// Make an organization, with you as its owner.
    Create(Creating),
    /// The organizations you are a member of, with your role in each.
    List(Server),
    /// Who is in an organization, and add, remove or change them.
    Member(Member),
}

/// `ink org create <NAME>`.
#[derive(Debug, Args)]
pub struct Creating {
    /// What it is called: one line, at most 64 characters.
    pub name: OrgName,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `ink org member <list|add|remove|role>`.
#[derive(Debug, Args)]
pub struct Member {
    /// What to do with the members.
    #[command(subcommand)]
    pub command: Members,
}

/// What `ink org member` does.
#[derive(Debug, Subcommand)]
pub enum Members {
    /// The people in an organization, owners first.
    List(Whose),
    /// Add a person by their account id.
    Add(Adding),
    /// Remove a person. The keys they approved stay with the organization.
    Remove(Removing),
    /// Give a member another role.
    Role(Regrading),
}

/// A command about one organization.
#[derive(Debug, Args)]
pub struct Whose {
    /// The organization's id, as `ink org list` shows it.
    pub org: Id<AccountKind>,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `ink org member add <ORG> <ACCOUNT>`.
#[derive(Debug, Args)]
pub struct Adding {
    /// The organization's id.
    pub org: Id<AccountKind>,
    /// The person's account id.
    pub account: Id<AccountKind>,
    /// What they may do: owner, admin, writer or reader.
    #[arg(long, value_name = "ROLE", default_value = "reader")]
    pub role: Role,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `ink org member remove <ORG> <ACCOUNT>`.
#[derive(Debug, Args)]
pub struct Removing {
    /// The organization's id.
    pub org: Id<AccountKind>,
    /// The person's account id.
    pub account: Id<AccountKind>,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `ink org member role <ORG> <ACCOUNT> <ROLE>`.
#[derive(Debug, Args)]
pub struct Regrading {
    /// The organization's id.
    pub org: Id<AccountKind>,
    /// The person's account id.
    pub account: Id<AccountKind>,
    /// What they may do from now on: owner, admin, writer or reader.
    pub role: Role,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `GET /v1/orgs`: `{items: [...]}`.
#[derive(Debug, Deserialize)]
struct Items {
    items: Vec<OrgView>,
}

/// The body of `POST /v1/orgs`.
#[derive(Debug, Serialize)]
struct Naming<'a> {
    name: &'a str,
}

/// The body of `POST /v1/orgs/{org}/members`.
#[derive(Debug, Serialize)]
struct Seating {
    account: Id<AccountKind>,
    role: &'static str,
}

/// The body of `PUT /v1/orgs/{org}/members/{account}`.
#[derive(Debug, Serialize)]
struct Grading {
    role: &'static str,
}

/// What `ink org list --json` prints.
#[derive(Debug, Serialize)]
struct OrgsOut<'a> {
    schema: &'static str,
    orgs: &'a [OrgView],
}

/// What `ink org create --json` prints.
#[derive(Debug, Serialize)]
struct OrgOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    org: &'a OrgView,
}

/// What `ink org member list --json` prints.
#[derive(Debug, Serialize)]
struct MembersOut<'a> {
    schema: &'static str,
    org: &'a OrgView,
    members: &'a [MemberView],
}

/// What `ink org member add` and `role` print with `--json`.
#[derive(Debug, Serialize)]
struct MemberOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    member: &'a MemberView,
}

/// What `ink org member remove --json` prints.
#[derive(Debug, Serialize)]
struct GoneOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    gone: &'a Unmembered,
}

/// What the list of organizations says when you are in none.
const EMPTY: Empty = Empty {
    said: "You are in no organizations.",
    next: Some(("ink org create \"Garden club\"", "Make one; you are its owner")),
};

/// The organizations as a table.
#[derive(Debug)]
struct Listing<'a>(&'a [OrgView]);

impl Render for Listing<'_> {
    fn render(&self, into: &mut Doc) {
        let mut table = Listed::new(EMPTY, &["id", "name", "role", "members"]);
        for org in self.0 {
            table = table.row(vec![
                Line::new().with(Tone::Id, org.id.to_string()),
                Line::plain(org.name.clone()),
                Line::plain(org.role.clone()),
                Line::plain(org.members.to_string()),
            ]);
        }
        table.render(into);
    }
}

/// The members of an organization as a table.
#[derive(Debug)]
struct People<'a>(&'a [MemberView]);

impl Render for People<'_> {
    fn render(&self, into: &mut Doc) {
        let quiet = |text: &str| Line::new().with(Tone::Quiet, text);
        let mut table = Table::default().indent(2).row(["account", "role", "joined"].map(quiet).to_vec());
        for member in self.0 {
            table = table.row(vec![
                Line::new().with(Tone::Id, member.account.to_string()),
                Line::plain(member.role.clone()),
                Line::plain(Moment(member.since).to_string()),
            ]);
        }
        table.render(into);
    }
}

/// One organization as a record.
#[derive(Debug)]
struct Made<'a>(&'a OrgView);

impl Render for Made<'_> {
    fn render(&self, into: &mut Doc) {
        Record::new(0)
            .field("org", Line::new().with(Tone::Id, self.0.id.to_string()))
            .field("name", Line::plain(self.0.name.clone()))
            .field("role", Line::plain(self.0.role.clone()))
            .field("members", Line::plain(self.0.members.to_string()))
            .render(into);
    }
}

/// A member as a record.
#[derive(Debug)]
struct Seated<'a>(&'a MemberView);

impl Render for Seated<'_> {
    fn render(&self, into: &mut Doc) {
        Record::new(0)
            .field("account", Line::new().with(Tone::Id, self.0.account.to_string()))
            .field("role", Line::plain(self.0.role.clone()))
            .field("joined", Line::plain(Moment(self.0.since).to_string()))
            .render(into);
    }
}

/// A member who left.
#[derive(Debug)]
struct Left<'a>(&'a Unmembered);

impl Render for Left<'_> {
    fn render(&self, into: &mut Doc) {
        Record::new(0)
            .field("org", Line::new().with(Tone::Id, self.0.org.to_string()))
            .field("account", Line::new().with(Tone::Id, self.0.account.to_string()))
            .field("state", Line::new().with(Tone::Good, "removed"))
            .render(into);
    }
}

impl Orgs {
    /// Runs the command.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        let encoded = |source| Failure::Unreadable { source };
        match self {
            Self::Create(creating) => {
                let calls = creating.server.signed(&Process, &preferences)?;
                let body = serde_json::to_vec(&Naming { name: creating.name.as_str() }).map_err(encoded)?;
                let org: OrgView = calls.json(Method::POST, "/v1/orgs", body)?;
                io.result(&Made(&org))?;
                io.json(&OrgOut { schema: "uses.org/v1", org: &org })?;
            }
            Self::List(server) => {
                let calls = server.signed(&Process, &preferences)?;
                let items: Items = calls.json(Method::GET, "/v1/orgs", Vec::new())?;
                io.result(&Listing(&items.items))?;
                io.json(&OrgsOut { schema: "uses.orgs/v1", orgs: &items.items })?;
            }
            Self::Member(member) => return member.command.run(io, &preferences),
        }
        Ok(Exit::Done)
    }
}

impl Members {
    fn run(self, io: &mut Io<'_>, preferences: &Preferences) -> Result<Exit, Error> {
        let encoded = |source| Failure::Unreadable { source };
        match self {
            Self::List(whose) => {
                let calls = whose.server.signed(&Process, preferences)?;
                let shown: OrgMembers = calls.json(Method::GET, &format!("/v1/orgs/{}", whose.org), Vec::new())?;
                io.result(&Made(&shown.org))?;
                io.result(&People(&shown.members))?;
                io.json(&MembersOut { schema: "uses.org-members/v1", org: &shown.org, members: &shown.members })?;
            }
            Self::Add(adding) => {
                let calls = adding.server.signed(&Process, preferences)?;
                let body = serde_json::to_vec(&Seating { account: adding.account, role: adding.role.name() })
                    .map_err(encoded)?;
                let path = format!("/v1/orgs/{}/members", adding.org);
                let member: MemberView = calls.json(Method::POST, &path, body)?;
                io.result(&Seated(&member))?;
                io.json(&MemberOut { schema: "uses.org-member/v1", member: &member })?;
            }
            Self::Role(regrading) => {
                let calls = regrading.server.signed(&Process, preferences)?;
                let body = serde_json::to_vec(&Grading { role: regrading.role.name() }).map_err(encoded)?;
                let path = format!("/v1/orgs/{}/members/{}", regrading.org, regrading.account);
                let member: MemberView = calls.json(Method::PUT, &path, body)?;
                io.result(&Seated(&member))?;
                io.json(&MemberOut { schema: "uses.org-member/v1", member: &member })?;
            }
            Self::Remove(removing) => {
                let calls = removing.server.signed(&Process, preferences)?;
                let path = format!("/v1/orgs/{}/members/{}", removing.org, removing.account);
                let gone: Unmembered = calls.json(Method::DELETE, &path, Vec::new())?;
                io.result(&Left(&gone))?;
                io.json(&GoneOut { schema: "uses.org-removed/v1", gone: &gone })?;
            }
        }
        Ok(Exit::Done)
    }
}
