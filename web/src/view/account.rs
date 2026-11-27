//! The Account screen, outside any site: the person's sites, how they are signed in, the export
//! of a site, and its removal.

use std::fmt;

use control::{Handle, SiteState};
use jiff::Timestamp;
use unspliced::keyword::{ButtonType, InputType};
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Empty, Link, Post, Stamp};
use crate::view::passkeys::{Kept, Section};
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
}

impl Account {
    /// An Account screen.
    pub fn new(created: Timestamp, sites: Vec<Owned>) -> Self {
        Self { frame: Frame::account(), created, sites, passkeys: Vec::new() }
    }

    /// The same screen listing `passkeys`.
    #[must_use]
    pub fn with_passkeys(self, passkeys: Vec<Kept>) -> Self {
        Self { passkeys, ..self }
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
        html! {
            p.look::LEDE { "Account made " (Stamp(self.created)) "." }
            h2 { "Your sites" }
            @if self.sites.is_empty() {
                (Empty { title: "No site yet", what: "Claim a handle and point it at your content with `ink push` from the folder you write in. Your site then shows up here." })
            } @else {
                @for owned in &self.sites { (Lazy(move |out: &mut dyn Sink| Self::site(owned, out))) }
            }
            h2 { "Sign-in" }
            p { "You can sign in with an email link or with a passkey." }
            (Section(&self.passkeys))
            h2 { "Notifications" }
            p { "Failure notices are sent by email: the first failure and its fix only." }
        }
        .render(out)
    }
}

impl Account {
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
