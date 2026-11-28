//! The two pages around an email link: what is said after one is asked for, and the page the
//! link opens. Opening it spends nothing; the button on it does, so a mail scanner that opens
//! every link in a message cannot use one up.

use std::fmt;

use unspliced::keyword::{ButtonType, InputType};
use unspliced::{Halt, Render, Sink, html};

use crate::form::Token;
use crate::route::Path;
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Link, Post};

/// What is said after a link was asked for: the same words for every address, so the page
/// tells nobody which addresses have an account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
    frame: Frame,
}

impl Sent {
    /// The page.
    pub const fn new() -> Self {
        Self { frame: Frame { site: None, here: None } }
    }
}

impl Default for Sent {
    fn default() -> Self {
        Self::new()
    }
}

impl View for Sent {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "Check your email"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let back = Path::signin();
        html! {
            p.look::LEDE { "If that address has an account, a sign-in link is on its way. It works once, and only for a short while." }
            p { "Nothing arrived? Look in the spam folder, then ask again on " (Link(&back, "the sign-in page")) "." }
        }
        .render(out)
    }
}

/// The page a sign-in link opens: one button that signs in, and a field for the address that the
/// first-run link asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entering {
    frame: Frame,
    /// The link's secret, written back into the form that posts it.
    pub token: Token,
}

impl Entering {
    /// The page of the link whose secret is `token`.
    pub const fn new(token: Token) -> Self {
        Self { frame: Frame { site: None, here: None }, token }
    }
}

impl View for Entering {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "Sign in"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let to = Path::login(&self.token);
        html! {
            p.look::LEDE { "This link signs you in to this browser, once." }
            (Post { to: &to, name: "Sign in with this link", body: html! {
                div.look::STACK {
                    label { "Email address, only if this is the first-run link " input type=(InputType::Email) name="address" autocomplete="email"; }
                    button.look::MAIN type=(ButtonType::Submit) { "Sign in" }
                }
            } })
        }
        .render(out)
    }
}
