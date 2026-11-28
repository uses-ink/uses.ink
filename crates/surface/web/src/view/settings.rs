//! The Settings screen: where the site reads from and serves to. Read-only: interpretation lives
//! in `ink.toml` in the person's own source, so there is no second place to set it (decision
//! 0044, flow.md section 8).

use std::fmt;

use build::Source;
use control::{Handle, Purpose};
use jiff::Timestamp;
use model::ExternalUrl;
use unspliced::keyword::{ButtonType, InputType};
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::Path;
use crate::route::Screen;
use crate::shell::{Frame, Glyph, View};
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

/// Where the site's repository stands (decision 0160).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Followed {
    /// Pushes to it build the site, since this time.
    Proven(Timestamp),
    /// It waits for the challenge to be committed.
    Pending {
        /// The line to commit.
        challenge: String,
        /// The file it goes in.
        file: String,
    },
}

/// A repository the site claimed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// `host/owner/name`.
    pub repo: String,
    /// Where it stands.
    pub standing: Followed,
    /// The path of the webhook to give the forge, on the API host, when the server reaches the
    /// forge.
    pub hook: Option<String>,
}

/// A webhook secret just made, shown on the page the change answers with and on no other.
#[derive(Clone, PartialEq, Eq)]
pub struct Given {
    /// The repository.
    pub repo: String,
    /// The secret, in hex.
    pub secret: String,
}

impl fmt::Debug for Given {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Given").field("repo", &self.repo).field("secret", &"<redacted>").finish()
    }
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
    /// The repositories it claimed, proved and not.
    pub repos: Vec<Repository>,
    /// A webhook secret made by the change this page answers.
    pub given: Option<Given>,
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
            repos: Vec::new(),
            given: None,
        }
    }

    /// The same screen showing a secret that was just made.
    #[must_use]
    pub fn with_given(self, given: Given) -> Self {
        Self { given: Some(given), ..self }
    }
}

/// What a domain's form does: the action it posts to and the words on its button.
#[derive(Clone, Copy, Debug)]
struct Button {
    action: &'static str,
    label: &'static str,
}

impl Button {
    const STOP: Self = Self { action: "unclaim", label: "Stop serving this name" };
    const PROVE: Self = Self { action: "prove", label: "Check the record" };
    const DROP: Self = Self { action: "unclaim", label: "Drop this claim" };
}

impl Settings {
    /// One form that posts the name of a domain to `action`.
    fn named(&self, button: Button, name: &str, out: &mut dyn Sink) -> Result<(), Halt> {
        let (to, label) = (Path::act(&self.handle, button.action), button.label);
        (Post {
            to: &to,
            name: label,
            body: html! {
                input type=(InputType::Hidden) name="name" value=(Escaped(name));
                button type=(ButtonType::Submit) { (label) }
            },
        })
        .render(out)
    }

    /// One form that posts to the repository `action` and has no fields.
    fn step(&self, action: &'static str, label: &'static str, out: &mut dyn Sink) -> Result<(), Halt> {
        let to = Path::act(&self.handle, action);
        (Post { to: &to, name: label, body: html! { button type=(ButtonType::Submit) { (label) } } }).render(out)
    }

    fn repos(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            @if let Some(given) = &self.given {
                div.look::BANNER role="status" { p { "The webhook secret of " code { (Escaped(&given.repo)) } " is " code { (Escaped(&given.secret)) } ". It is shown here once and is kept sealed: give it to the forge now, or rotate it later." } }
            }
            @if self.repos.is_empty() {
                (Empty { title: "No repository claimed", what: "Claim the site's repository to build it when its forge says it moved, or with `ink repo claim`." })
                (Lazy(|out: &mut dyn Sink| self.step("repo-claim", "Claim the repository", out)))
            } @else {
                @for repo in &self.repos {
                    div.look::CARD {
                        p.look::HEAD { code.look::BIG { (Escaped(&repo.repo)) } @match &repo.standing {
                            Followed::Proven(_) => { span.look::STATE.look::LIVE { (Glyph("ok", "builds on every push")) } },
                            Followed::Pending { .. } => { span.look::STATE.look::PAUSED { (Glyph("...", "waiting for proof")) } },
                        } }
                        @match &repo.standing {
                            Followed::Proven(at) => {
                                p { "Builds the site on every push since " (Stamp(*at)) "." @if let Some(hook) = &repo.hook { " The forge posts to " code { (Escaped(hook)) } " on the API host." } }
                                (Lazy(|out: &mut dyn Sink| self.step("repo-rotate", "Rotate the webhook secret", out)))
                            },
                            Followed::Pending { challenge, file } => {
                                p { "Commit this line to this file on its default branch, then ask for the proof:" }
                                dl.look::RECORD { div { dt { "line" } dd { code { (Escaped(challenge)) } } } div { dt { "file" } dd { code { (Escaped(file)) } } } }
                                (Lazy(|out: &mut dyn Sink| self.step("repo-prove", "Check the repository", out)))
                                (Lazy(|out: &mut dyn Sink| self.step("repo-claim", "Ask again", out)))
                            },
                        }
                        (Lazy(|out: &mut dyn Sink| self.step("repo-release", "Let go of the repository", out)))
                    }
                }
            }
        }
        .render(out)
    }

    fn domains(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let claim = Path::act(&self.handle, "claim");
        html! {
            @if self.domains.is_empty() {
                (Empty { title: "No custom domain", what: "The site is served at its address above. Claim a name below, or with `ink domain add`, and publish the record it shows." })
            } @else {
                @for custom in &self.domains {
                    div.look::CARD {
                        p.look::HEAD { code.look::BIG { (Escaped(&custom.name)) } @match &custom.standing {
                            Standing::Proven(_) => { span.look::STATE.look::LIVE { (Glyph("ok", "serving")) } },
                            Standing::Pending { .. } => { span.look::STATE.look::PAUSED { (Glyph("...", "waiting for DNS")) } },
                        } }
                        @match &custom.standing {
                            Standing::Proven(at) => {
                                p { "Serves this site since " (Stamp(*at)) "." }
                                (Lazy(|out: &mut dyn Sink| self.named(Button::STOP, &custom.name, out)))
                            },
                            Standing::Pending { host, value } => {
                                p { "Add this record where your domain's DNS lives, then ask for the proof:" }
                                dl.look::RECORD { div { dt { "type" } dd { code { "TXT" } } } div { dt { "name" } dd { code { (Escaped(host)) } } } div { dt { "value" } dd { code { (Escaped(value)) } } } }
                                (Lazy(|out: &mut dyn Sink| self.named(Button::PROVE, &custom.name, out)))
                                (Lazy(|out: &mut dyn Sink| self.named(Button::DROP, &custom.name, out)))
                            },
                        }
                    }
                }
            }
            (Post { to: &claim, name: "Claim a domain", body: html! {
                div.look::STACK {
                    label { "Domain name " input type=(InputType::Text) name="name" autocomplete="off" placeholder="www.example.org" required; }
                    button.look::MAIN type=(ButtonType::Submit) { "Claim the name" }
                }
            } })
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
            (Lazy(|out: &mut dyn Sink| self.domains(out)))
            h2 { "Repository" }
            (Lazy(|out: &mut dyn Sink| self.repos(out)))
            h2 { "Theme" }
            p { "The theme is read from your source. To change it, commit this to " code.look::PATH { "ink.toml" } ":" }
            pre { code { "[theme]\nname = \"quiet\"" } }
            h2 { "Publishing" }
            @match &self.serving {
                Serving::Paused(_) => {
                    p { "Paused after a rollback. New builds are kept as previews." }
                    (Post { to: &resume, name: "Resume publishing", body: html! {
                        button.look::MAIN type=(ButtonType::Submit) { "Resume publishing" }
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
