//! The Account screen, outside any site: the person's sites, how they are signed in, the export
//! of a site, and its removal.

use std::fmt;

use control::{Handle, Role, SiteState};
use jiff::Timestamp;
use unspliced::keyword::{ButtonType, InputType};
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Empty, Link, Post, Stamp};
use crate::view::passkeys::{Kept, Section};
use crate::view::recovery::Section as Recovery;
use crate::view::sessions::{Section as Sessions, Seen};
use crate::view::word::Word;

/// One of the person's sites.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    /// Its handle.
    pub handle: Handle,
    /// Whether it takes changes.
    pub state: SiteState,
    /// When it was made.
    pub created: Timestamp,
}

/// An organization the person is a member of (decision 0095).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joined {
    /// What people call it.
    pub name: String,
    /// What the person may do there.
    pub role: Role,
    /// How many members it has.
    pub members: u64,
}

/// What the Account screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// Where the page sits.
    pub frame: Frame,
    /// When the account was made.
    pub created: Timestamp,
    /// The person's sites.
    pub sites: Vec<Owned>,
    /// The person's passkeys.
    pub passkeys: Vec<Kept>,
    /// The person's signed-in sessions.
    pub sessions: Vec<Seen>,
    /// How many recovery codes are unspent, when the instance has them.
    pub recovery: Option<u64>,
    /// The organizations the person is a member of.
    pub orgs: Vec<Joined>,
}

impl Account {
    /// An Account screen.
    pub fn new(created: Timestamp, sites: Vec<Owned>) -> Self {
        Self {
            frame: Frame::account(),
            created,
            sites,
            passkeys: Vec::new(),
            sessions: Vec::new(),
            recovery: None,
            orgs: Vec::new(),
        }
    }

    /// The same screen listing `passkeys`.
    #[must_use]
    pub fn with_passkeys(self, passkeys: Vec<Kept>) -> Self {
        Self { passkeys, ..self }
    }

    /// The same screen saying `left` recovery codes are unspent.
    #[must_use]
    pub fn with_recovery(self, left: u64) -> Self {
        Self { recovery: Some(left), ..self }
    }

    /// The same screen listing the `orgs` the person belongs to.
    #[must_use]
    pub fn with_orgs(self, orgs: Vec<Joined>) -> Self {
        Self { orgs, ..self }
    }

    /// The same screen listing `sessions`.
    #[must_use]
    pub fn with_sessions(self, sessions: Vec<Seen>) -> Self {
        Self { sessions, ..self }
    }
}

impl View for Account {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "Account"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let logout = Path::logout();
        html! {
            p.look::LEDE { "Account made " (Stamp(self.created)) "." }
            h2 { "Your sites" }
            @if self.sites.is_empty() {
                (Empty { title: "No site yet", what: "Claim a handle and point it at your content with `ink push` from the folder you write in. Your site then shows up here." })
            } @else {
                @for owned in &self.sites { (Lazy(move |out: &mut dyn Sink| Self::site(owned, out))) }
            }
            @if !self.orgs.is_empty() {
                (Lazy(|out: &mut dyn Sink| self.organizations(out)))
            }
            h2 { "Sign-in" }
            p { "You can sign in with an email link or with a passkey." }
            (Section(&self.passkeys))
            @if let Some(left) = self.recovery { (Recovery(left)) }
            (Sessions(&self.sessions))
            (Post { to: &logout, name: "Sign out", body: html! {
                p { button type=(ButtonType::Submit) { "Sign out of this session" } }
            } })
            h2 { "Notifications" }
            p { "Failure notices are sent by email: the first failure and its fix only." }
        }
        .render(out)
    }
}

impl Account {
    /// The organizations the person belongs to, with their role in each.
    fn organizations(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            h2 { "Organizations" }
            ul {
                @for joined in &self.orgs {
                    li { strong { (Escaped(&joined.name)) } ", you are " (Escaped(joined.role)) ", " (joined.members) " members" }
                }
            }
        }
        .render(out)
    }

    /// One site: a link to it, its export, and its removal behind a typed confirmation.
    fn site(owned: &Owned, out: &mut dyn Sink) -> Result<(), Halt> {
        let handle = &owned.handle;
        let (site, export, remove) =
            (Path::of(Screen::Site, handle), Path::export(handle), Path::act(handle, "remove"));
        let removing = owned.state == SiteState::Removing;
        html! {
            section.look::CARD aria-label=(Escaped(handle)) {
                h3 { (Link(&site, Escaped(handle))) }
                p.look::META { (Escaped(Word(owned.state))) ", made " (Stamp(owned.created)) }
                p { (Link(&export, "Export the site")) " as a folder of files you can host anywhere." }
                details {
                    summary { "Remove this site" }
                    div.look::DANGER { p { "This removes the site, its deploys and its stored files. The handle stays reserved. " strong { "It cannot be undone." } @if removing { " A removal began earlier; removing again finishes it." } }
                    (Post { to: &remove, name: "Remove the site", body: html! {
                        div.look::STACK {
                            label { "Type " code.look::PATH { (Escaped(handle)) } " to confirm"
                                input type=(InputType::Text) name="confirm" autocomplete="off" required;
                            }
                            button.look::DANGER type=(ButtonType::Submit) { "Remove " (Escaped(handle)) }
                        }
                    } }) }
                }
            }
        }
        .render(out)
    }
}
