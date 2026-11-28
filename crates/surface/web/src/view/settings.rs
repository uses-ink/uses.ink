//! The Settings screen: where the site reads from and serves to. Read-only: interpretation lives
//! in `ink.toml` in the person's own source, so there is no second place to set it (decision
//! 0044, flow.md section 8).

use std::fmt;

use build::Source;
use control::{Handle, Purpose};
use jiff::Timestamp;
use model::ExternalUrl;
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::Path;
use crate::route::Screen;
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Empty, Outside, Post, Serving, Stamp};
use crate::view::understood::Describe;
use crate::view::word::Word;

/// A credential the site holds, without the credential: nothing here can be printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    /// What it is for.
    pub purpose: Purpose,
    /// When it was stored.
    pub created: Timestamp,
}

/// Where a custom domain stands.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Standing {
    /// The name serves the site since this time.
    Proven(Timestamp),
    /// The name waits for a TXT record: where to publish it and what it says.
    Pending {
        /// The host of the record.
        host: String,
        /// Its value.
        value: String,
    },
}

/// A custom domain of the site (decision 0093).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custom {
    /// The name.
    pub name: String,
    /// Where it stands.
    pub standing: Standing,
}

/// What the Settings screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// Where it is served.
    pub address: ExternalUrl,
    /// Where the content comes from.
    pub source: Source,
    /// What it serves now, and whether publishing is paused.
    pub serving: Serving,
    /// The credentials its source needs.
    pub credentials: Vec<Held>,
    /// The custom domains it serves or has still to prove.
    pub domains: Vec<Custom>,
}

impl Settings {
    /// A Settings screen for `handle`.
    pub fn new(handle: Handle, address: ExternalUrl, source: Source, serving: Serving) -> Self {
        Self {
            frame: Frame::of(&handle, Screen::Settings),
            handle,
            address,
            source,
            serving,
            credentials: Vec::new(),
            domains: Vec::new(),
        }
    }
}

impl Settings {
    fn named(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            @if self.domains.is_empty() {
                (Empty { title: "No custom domain", what: "The site is served at its address above. Custom domains are added with `ink domain add`; they show up here once verified." })
            } @else {
                ul {
                    @for custom in &self.domains {
                        li {
                            code { (Escaped(&custom.name)) }
                            @match &custom.standing {
                                Standing::Proven(at) => { " serves this site since " (Stamp(*at)) "." },
                                Standing::Pending { host, value } => {
                                    " is waiting for proof. Publish a TXT record at " code { (Escaped(host)) } " with the value " code { (Escaped(value)) } ", then ask for the proof again."
                                },
                            }
                        }
                    }
                }
            }
        }
        .render(out)
    }
}

impl View for Settings {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "Settings"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let address = self.address.to_string();
        let resume = Path::act(&self.handle, "resume");
        html! {
            p.look::LEDE { "Where this site reads from and where it is served. How your content is read is set in " code.look::PATH { "ink.toml" } " in your source, so nothing here changes it." }
            h2 { "Source" }
            p { (Escaped(Describe(&self.source))) }
            h2 { "Address" }
            p { (Outside(&address)) }
            h2 { "Domains" }
            (Lazy(|out: &mut dyn Sink| self.named(out)))
            h2 { "Theme" }
            p { "The theme is read from your source. To change it, commit this to " code.look::PATH { "ink.toml" } ":" }
            pre { code { "[theme]\nname = \"quiet\"" } }
            h2 { "Publishing" }
            @match &self.serving {
                Serving::Paused(_) => {
                    p { "Paused after a rollback. New builds are kept as previews." }
                    (Post { to: &resume, name: "Resume publishing", body: html! {
                        button.look::MAIN type=(unspliced::keyword::ButtonType::Submit) { "Resume publishing" }
                    } })
                },
                Serving::Live(_) => { p { "Automatic: every build that changes the site is published." } },
                Serving::Nothing => { p { "Automatic, once a build has made a deploy." } },
            }
            h2 { "Credentials" }
            @if self.credentials.is_empty() {
                (Empty { title: "No credentials", what: "A public source needs none. A private git repository needs a token: add one with `ink source add --token`." })
            } @else {
                ul {
                    @for held in &self.credentials {
                        li { (Escaped(Word(held.purpose))) ", stored " (Stamp(held.created)) ". The value is sealed and is never shown." }
                    }
                }
            }
        }
        .render(out)
    }
}
