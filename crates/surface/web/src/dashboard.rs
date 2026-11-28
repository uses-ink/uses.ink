//! The edge's side of the dashboard: the trait it implements over the control plane, and the
//! one call that turns a request into a reply. The crate knows no HTTP server: the edge reads
//! its request into a [`Request`], calls [`Dashboard::respond`], and sends the [`Reply`].

use std::fmt;
use std::future::Future;

use control::{AccountKind, Handle, Id};

use crate::form::{Action, Asked, Token};
use crate::route::{Only, Path, Route, Screen};
use crate::shell::{Document, Frame, View};
use crate::stream::{Follow, Streamed};
use crate::view::{
    Account, Build, Deploys, Entering, Failed, Limits, Live, Sent, Settings, SignIn, Site, Stored, Understood, Why,
};
use crate::{Failure, Request, Status};

/// What a request for a build's log finds.
#[derive(Debug)]
#[non_exhaustive]
pub enum Logged<F> {
    /// The build ended: its whole log.
    Done(Build),
    /// The build's log as stored, which an edge that holds no typed events shows, ended or not.
    Stored(Stored),
    /// The build is running: the head of its page and the events to follow.
    Running(Live, F),
}

/// A site exported, to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Download {
    /// The site exported.
    pub of: Handle,
    /// The archive.
    pub bytes: Vec<u8>,
}

impl fmt::Display for Download {
    /// The file name offered: `<handle>.export`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}.export", self.of)
    }
}

/// What the edge provides: each screen's data from the control plane, for the account that is
/// signed in, and the changes forms ask for. An implementation checks that the site belongs to
/// the account and answers [`Failure::Foreign`] when it does not. Its futures need not be `Send`: the
/// control plane's ports are not on every host, so an edge that holds them awaits these on the task
/// that owns them.
pub trait Screens {
    /// Where a running build's events come from.
    type Follow: Follow;

    /// The limits this server enforces, which the public limits page lists.
    fn limits(&self) -> control::Limits;

    /// The Site screen.
    fn site(&self, who: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Site, Failure>>;

    /// The What we understood screen.
    fn understood(
        &self,
        who: Id<AccountKind>,
        handle: &Handle,
        only: Only,
    ) -> impl Future<Output = Result<Understood, Failure>>;

    /// The replay of one decision.
    fn why(&self, who: Id<AccountKind>, handle: &Handle, asked: &Asked) -> impl Future<Output = Result<Why, Failure>>;

    /// The Deploys screen.
    fn deploys(&self, who: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Deploys, Failure>>;

    /// The Settings screen.
    fn settings(&self, who: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Settings, Failure>>;

    /// The Account screen.
    fn account(&self, who: Id<AccountKind>) -> impl Future<Output = Result<Account, Failure>>;

    /// Ends one of the account's sessions. A session that is another account's, has ended or
    /// never was is answered as an address that names nothing, [`Failure::Unrouted`].
    fn end_session(
        &self,
        who: Id<AccountKind>,
        id: Id<control::SessionKind>,
    ) -> impl Future<Output = Result<(), Failure>>;

    /// Asks for a sign-in link to be sent to `address`. The answer is the same whether or not the
    /// address has an account.
    fn ask(&self, address: &str) -> impl Future<Output = Result<(), Failure>>;

    /// Uses the sign-in link `token`, which the first-run link gives `address`. The session it
    /// begins is the edge's to hand to the browser.
    fn enter(&self, token: &Token, address: Option<&str>) -> impl Future<Output = Result<(), Failure>>;

    /// Ends the session the request holds.
    fn sign_out(&self, who: Id<AccountKind>) -> impl Future<Output = Result<(), Failure>>;

    /// A build's log.
    fn log(
        &self,
        who: Id<AccountKind>,
        handle: &Handle,
        build: build::BuildId,
    ) -> impl Future<Output = Result<Logged<Self::Follow>, Failure>>;

    /// Makes the change a form asked for.
    fn act(&self, who: Id<AccountKind>, handle: &Handle, action: &Action) -> impl Future<Output = Result<(), Failure>>;

    /// The site as a download.
    fn export(&self, who: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Download, Failure>>;
}

/// What a reply carries.
#[derive(Debug)]
#[non_exhaustive]
pub enum Body<F> {
    /// Nothing: a redirect.
    Empty,
    /// A whole HTML page.
    Page(String),
    /// An HTML page sent in chunks, ending with the result.
    Streamed(Box<Streamed<F>>),
    /// A file to save.
    Download(Download),
    /// The dashboard's script, as static text with a JavaScript content type.
    Script(&'static str),
}

/// What the dashboard answers.
#[derive(Debug)]
pub struct Reply<F> {
    /// The status.
    pub status: Status,
    /// Where to go next, for a redirect.
    pub location: Option<Path>,
    /// What it carries.
    pub body: Body<F>,
}

impl<F> Reply<F> {
    /// The headers every dashboard reply carries: pages are private and never framed, and a page
    /// may style itself and post only to itself.
    pub const HEADERS: [(&'static str, &'static str); 4] = [
        ("cache-control", "no-store"),
        (
            "content-security-policy",
            "default-src 'none'; script-src 'self'; connect-src 'self'; style-src 'unsafe-inline'; img-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "same-origin"),
    ];

    fn page(status: Status, view: &(impl View + ?Sized)) -> Result<Self, Failure> {
        Ok(Self { status, location: None, body: Body::Page(Document(view).write()?) })
    }

    fn see(path: Path) -> Self {
        Self { status: Status::SeeOther, location: Some(path), body: Body::Empty }
    }
}

/// The dashboard over what the edge provides.
#[derive(Debug)]
pub struct Dashboard<S>(pub S);

impl<S: Screens> Dashboard<S> {
    /// The script the passkey buttons need: one static file, served from this host.
    pub const SCRIPT: &'static str = include_str!("passkey.js");

    /// Answers `request` from `who`, who is signed in or is not. A failure is a page with its
    /// code, cause and fix, and the status the failure says.
    pub async fn respond(&self, who: Option<Id<AccountKind>>, request: &Request<'_>) -> Reply<S::Follow> {
        let route = match Route::read(request) {
            Ok(route) => route,
            Err(failure) => return Self::failed(Frame { site: None, here: None }, failure),
        };
        let frame = Frame { site: route.handle().cloned(), here: route.screen() };
        match self.serve(who, &route).await {
            Ok(reply) => reply,
            Err(failure) => Self::failed(frame, failure),
        }
    }

    /// A failure as the page that tells it, with the status it says, for an edge that refuses a
    /// request before the dashboard sees it.
    pub fn failed(frame: Frame, failure: Failure) -> Reply<S::Follow> {
        let status = failure.status();
        let page = Failed::new(frame, failure);
        match Document(&page).write() {
            Ok(text) => Reply { status, location: None, body: Body::Page(text) },
            Err(broken) => Reply { status: broken.status(), location: None, body: Body::Page(broken.to_string()) },
        }
    }

    #[expect(clippy::too_many_lines, reason = "one arm a route")]
    async fn serve(&self, who: Option<Id<AccountKind>>, route: &Route) -> Result<Reply<S::Follow>, Failure> {
        match route {
            Route::Home => {
                let to = if who.is_some() { Path::account() } else { Path::signin() };
                return Ok(Reply::see(to));
            }
            Route::SignIn => return Reply::page(Status::Ok, &SignIn::new()),
            Route::Limits => return Reply::page(Status::Ok, &Limits::new(self.0.limits())),
            Route::Sent => return Reply::page(Status::Ok, &Sent::new()),
            Route::Link(token) => return Reply::page(Status::Ok, &Entering::new(token.clone())),
            Route::Ask(address) => {
                self.0.ask(address).await?;
                return Ok(Reply::see(Path::sent()));
            }
            Route::Enter(token, address) => {
                self.0.enter(token, address.as_deref()).await?;
                return Ok(Reply::see(Path::account()));
            }
            Route::Script => {
                return Ok(Reply { status: Status::Ok, location: None, body: Body::Script(Self::SCRIPT) });
            }
            _ => {}
        }
        let who = who.ok_or(Failure::SignedOut)?;
        let screens = &self.0;
        match route {
            Route::Site(handle) => Reply::page(Status::Ok, &screens.site(who, handle).await?),
            Route::Understood(handle, only) => Reply::page(Status::Ok, &screens.understood(who, handle, *only).await?),
            Route::Why(handle, asked) => Reply::page(Status::Ok, &screens.why(who, handle, asked).await?),
            Route::Deploys(handle) => Reply::page(Status::Ok, &screens.deploys(who, handle).await?),
            Route::Settings(handle) => Reply::page(Status::Ok, &screens.settings(who, handle).await?),
            Route::Account => Reply::page(Status::Ok, &screens.account(who).await?),
            Route::SignOut => {
                screens.sign_out(who).await?;
                Ok(Reply::see(Path::signin()))
            }
            Route::Home
            | Route::SignIn
            | Route::Limits
            | Route::Script
            | Route::Sent
            | Route::Link(_)
            | Route::Ask(_)
            | Route::Enter(..) => Err(Failure::Unrouted { source: crate::Unrouted::Path }),
            Route::Build(handle, build) => match screens.log(who, handle, *build).await? {
                Logged::Done(page) => Reply::page(Status::Ok, &page),
                Logged::Stored(page) => Reply::page(Status::Ok, &page),
                Logged::Running(live, follow) => Ok(Reply {
                    status: Status::Ok,
                    location: None,
                    body: Body::Streamed(Box::new(Streamed::new(live, follow))),
                }),
            },
            Route::Export(handle) => Ok(Reply {
                status: Status::Ok,
                location: None,
                body: Body::Download(screens.export(who, handle).await?),
            }),
            Route::EndSession(id) => {
                screens.end_session(who, *id).await?;
                Ok(Reply::see(Path::account()))
            }
            Route::Act(handle, action) => {
                screens.act(who, handle, action).await?;
                Ok(Reply::see(Self::after(handle, action)))
            }
        }
    }

    /// Where a change sends the person: to the page it changed.
    fn after(handle: &Handle, action: &Action) -> Path {
        match action {
            Action::Build | Action::Rollback { .. } => Path::of(Screen::Deploys, handle),
            Action::Resume => Path::of(Screen::Site, handle),
            Action::Remove { .. } => Path::account(),
            Action::Claim { .. } | Action::Prove { .. } | Action::Unclaim { .. } => Path::of(Screen::Settings, handle),
        }
    }
}
