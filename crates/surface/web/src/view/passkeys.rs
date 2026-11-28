//! Passkeys on the dashboard: the list with its add and remove buttons, and the sign-in page.
//! Both need the one script (`/passkey.js`), because WebAuthn is a browser API; without it they
//! say so and name the other ways in.

use std::fmt;

use jiff::Timestamp;
use unspliced::keyword::{ButtonType, InputType};
use unspliced::{Halt, Lazy, Render, Sink, Url, html};

use crate::route::Path;
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Post, Stamp};

/// One passkey of the person, as the Account screen lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    /// Its id, which the remove button names.
    pub id: String,
    /// The name the person gave it.
    pub label: String,
    /// When it was registered.
    pub created: Timestamp,
    /// The day it last signed in.
    pub used: Option<String>,
    /// Whether it may be synced between devices.
    pub synced: bool,
}

/// The passkeys section of the Account screen.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Section<'a>(pub(crate) &'a [Kept]);

impl Render for Section<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            h2 { "Passkeys" }
            @if self.0.is_empty() {
                p { "No passkey yet. Add one to sign in without an email link." }
            } @else {
                ul.look::STACK {
                    @for kept in self.0 { li { (Lazy(move |out: &mut dyn Sink| Self::row(kept, out))) } }
                }
            }
            div.look::STACK {
                label { "Name for the new passkey " input type=(InputType::Text) data-passkey-label="" maxlength="64" value="This device"; }
                button type=(ButtonType::Button) data-passkey="register" { "Add a passkey" }
                p data-passkey-status="" role="status" {}
                noscript { p { "Passkeys need JavaScript. Without it, sign in with an email link or `ink login`." } }
            }
            script src=(Url::new("/passkey.js")) {}
        }
        .render(out)
    }
}

impl Section<'_> {
    fn row(kept: &Kept, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            strong { (&kept.label) }
            " made " (Stamp(kept.created))
            @if let Some(day) = &kept.used { ", last used " (day) }
            @if kept.synced { ", synced between devices" }
            " "
            button type=(ButtonType::Button) data-passkey="remove" data-passkey-id=(&kept.id) { "Remove" }
        }
        .render(out)
    }
}

/// The sign-in page: one button, and the other ways in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignIn {
    frame: Frame,
}

impl SignIn {
    /// The sign-in page.
    pub const fn new() -> Self {
        Self { frame: Frame { site: None, here: None } }
    }
}

impl Default for SignIn {
    fn default() -> Self {
        Self::new()
    }
}

impl View for SignIn {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "Sign in"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let asking = Path::signin();
        html! {
            p.look::LEDE { "Use a passkey from this device, your phone or a security key. No username is asked for." }
            div.look::STACK {
                button type=(ButtonType::Button) data-passkey="login" { "Sign in with a passkey" }
                p data-passkey-status="" role="status" {}
            }
            noscript { p { "Passkeys need JavaScript. Without it, use the sign-in link in your email, or `ink login` from a terminal." } }
            h2 { "Or get a link by email" }
            p { "The link signs you in once, from this browser. The page answers the same whether or not the address has an account." }
            (Post { to: &asking, name: "Email me a sign-in link", body: html! {
                div.look::STACK {
                    label { "Email address " input type=(InputType::Email) name="address" autocomplete="email" required; }
                    button.look::MAIN type=(ButtonType::Submit) { "Email me a link" }
                }
            } })
            script src=(Url::new("/passkey.js")) {}
        }
        .render(out)
    }
}
