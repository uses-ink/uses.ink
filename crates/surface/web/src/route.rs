//! Where the dashboard's pages are. `/<handle>` is the Site screen; with one site per handle
//! there is no list of sites to click through (flow.md, section 8).

use std::fmt::{self, Write as _};

use build::BuildId;
use control::{Handle, Id, SessionKind};

use crate::asset::Asset;
use crate::failure::Unrouted;
use crate::form::{Action, Asked, Form, Method, Request, Seating, Segment, Token};
use crate::{Failure, Field};

/// The five screens, in the order the navigation lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Screen {
    /// Is it live, and what changed?
    Site,
    /// Why does it look like this?
    Understood,
    /// What happened when?
    Deploys,
    /// Where does it read from and serve to?
    Settings,
    /// The person: sites, identities, export, delete.
    Account,
}

impl Screen {
    /// The screens of a site, in navigation order.
    pub const OF_SITE: [Self; 4] = [Self::Site, Self::Understood, Self::Deploys, Self::Settings];

    /// What the navigation calls it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Site => "Site",
            Self::Understood => "What we understood",
            Self::Deploys => "Deploys",
            Self::Settings => "Settings",
            Self::Account => "Account",
        }
    }
}

/// How much of the decisions the "What we understood" screen lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Only {
    /// Every decision.
    #[default]
    Everything,
    /// The decisions to check.
    Check,
}

/// An address the dashboard answers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Route {
    /// `/<handle>`.
    Site(Handle),
    /// `/<handle>/understood`, optionally `?only=check`.
    Understood(Handle, Only),
    /// `/<handle>/understood/why?code=page.title&on=posts/x.md`.
    Why(Handle, Asked),
    /// `/<handle>/deploys`.
    Deploys(Handle),
    /// `/<handle>/deploys/<build>`: the build's log, streamed while it runs.
    Build(Handle, BuildId),
    /// `/<handle>/settings`.
    Settings(Handle),
    /// `/account`.
    Account,
    /// `/signin`: the sign-in page, for someone who is not signed in: a passkey, or an email link.
    SignIn,
    /// `/limits`: what the server limits, public.
    Limits,
    /// `/`: the Account screen for someone signed in, the sign-in page for someone who is not.
    Home,
    /// A form posted to `/signin`: a sign-in link is asked for an address.
    Ask(String),
    /// `/signin/sent`: what is said after asking, the same for every address.
    Sent,
    /// `/login/<token>`: the page a sign-in link opens. It spends nothing until it is posted, so
    /// a mail scanner that opens every link cannot use one up.
    Link(Token),
    /// A form posted to `/login/<token>`: use the link; the first-run link is given an address.
    Enter(Token, Option<String>),
    /// A form posted to `/logout`: end the session the request holds.
    SignOut,
    /// `/passkey.js`: the dashboard's one script, static text.
    Script,
    /// `/<file>`: a face, the mark or a stroke the pages borrow.
    Asset(Asset),
    /// `/<handle>/export`: the site as a download.
    Export(Handle),
    /// A form posted to `/<handle>/<action>`; answered by a redirect to the page it changed.
    Act(Handle, Action),
    /// A form posted to `/account/sessions/<id>`: end one of the person's sessions.
    EndSession(Id<SessionKind>),
    /// A form posted to `/account/orgs`, `members`, `roles` or `removals`: make an organization or
    /// change who is in one.
    Seat(Seating),
}

impl Route {
    /// The address of the request, or why it is none of ours.
    #[expect(clippy::too_many_lines, reason = "one arm a path shape")]
    pub fn read(request: &Request<'_>) -> Result<Self, Failure> {
        let mut parts = request.path.trim_matches('/').split('/');
        let (first, second, third, rest) = (parts.next(), parts.next(), parts.next(), parts.next());
        if rest.is_some() {
            return Err(Failure::Unrouted { source: Unrouted::Path });
        }
        let first = first.filter(|text| !text.is_empty());
        let get = |route: Self| match request.method {
            Method::Get => Ok(route),
            Method::Post => Err(Failure::Method { used: Method::Post }),
        };
        let post = |name: &str, handle: Handle| match request.method {
            Method::Post => Action::read(name, &request.body).map(|action| Self::Act(handle, action)),
            Method::Get => Err(Failure::Method { used: Method::Get }),
        };
        if let Some(asset) = first.filter(|_| second.is_none()).and_then(Asset::named) {
            return get(Self::Asset(asset));
        }
        match (first, second, third) {
            (Some("account"), None, None) => get(Self::Account),
            (Some("account"), Some(name @ ("orgs" | "members" | "roles" | "removals")), None) => match request.method {
                Method::Post => Seating::read(name, &request.body).map(Self::Seat),
                Method::Get => Err(Failure::Method { used: Method::Get }),
            },
            (Some("account"), Some("sessions"), Some(id)) => match request.method {
                Method::Post => Segment(id).session().map(Self::EndSession),
                Method::Get => Err(Failure::Method { used: Method::Get }),
            },
            (Some("signin"), None, None) => match request.method {
                Method::Get => Ok(Self::SignIn),
                Method::Post => Ok(Self::Ask(request.body.need(Field::Address)?.trim().to_owned())),
            },
            (Some("signin"), Some("sent"), None) => get(Self::Sent),
            (Some("login"), Some(token), None) => {
                let token = Segment(token).token()?;
                match request.method {
                    Method::Get => Ok(Self::Link(token)),
                    Method::Post => {
                        let given = request.body.value(Field::Address).map(str::trim).filter(|text| !text.is_empty());
                        Ok(Self::Enter(token, given.map(str::to_owned)))
                    }
                }
            }
            (Some("logout"), None, None) => match request.method {
                Method::Post => Ok(Self::SignOut),
                Method::Get => Err(Failure::Method { used: Method::Get }),
            },

            (Some("limits"), None, None) => get(Self::Limits),
            (Some("passkey.js"), None, None) => get(Self::Script),
            (None, ..) => get(Self::Home),
            (Some(handle), None, None) => get(Self::Site(Segment(handle).handle()?)),
            (Some(handle), Some("understood"), None) => {
                get(Self::Understood(Segment(handle).handle()?, Only::from(&request.query)))
            }
            (Some(handle), Some("understood"), Some("why")) => {
                get(Self::Why(Segment(handle).handle()?, Asked::read(&request.query)?))
            }
            (Some(handle), Some("deploys"), None) => get(Self::Deploys(Segment(handle).handle()?)),
            (Some(handle), Some("deploys"), Some(build)) => {
                get(Self::Build(Segment(handle).handle()?, Segment(build).build()?))
            }
            (Some(handle), Some("settings"), None) => get(Self::Settings(Segment(handle).handle()?)),
            (Some(handle), Some("export"), None) => get(Self::Export(Segment(handle).handle()?)),
            (Some(handle), Some(name), None) => post(name, Segment(handle).handle()?),
            _ => Err(Failure::Unrouted { source: Unrouted::Path }),
        }
    }

    /// The site the address is about, when it is about one.
    pub const fn handle(&self) -> Option<&Handle> {
        match self {
            Self::Account
            | Self::SignIn
            | Self::Limits
            | Self::Script
            | Self::Asset(_)
            | Self::EndSession(_)
            | Self::Seat(_)
            | Self::Home
            | Self::Ask(_)
            | Self::Sent
            | Self::Link(_)
            | Self::Enter(..)
            | Self::SignOut => None,
            Self::Site(handle)
            | Self::Understood(handle, _)
            | Self::Why(handle, _)
            | Self::Deploys(handle)
            | Self::Build(handle, _)
            | Self::Settings(handle)
            | Self::Export(handle)
            | Self::Act(handle, _) => Some(handle),
        }
    }

    /// The screen this address belongs to, when it is one.
    pub const fn screen(&self) -> Option<Screen> {
        match self {
            Self::Site(_) => Some(Screen::Site),
            Self::Understood(..) | Self::Why(..) => Some(Screen::Understood),
            Self::Deploys(_) | Self::Build(..) => Some(Screen::Deploys),
            Self::Settings(_) => Some(Screen::Settings),
            Self::Account | Self::EndSession(_) | Self::Seat(_) => Some(Screen::Account),
            Self::Export(_)
            | Self::Act(..)
            | Self::SignIn
            | Self::Limits
            | Self::Script
            | Self::Asset(_)
            | Self::Home
            | Self::Ask(_)
            | Self::Sent
            | Self::Link(_)
            | Self::Enter(..)
            | Self::SignOut => None,
        }
    }
}

/// A path to a screen, as a link writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path(String);

impl Path {
    /// The path of `screen` for the site `handle`; Account has none per site.
    pub fn of(screen: Screen, handle: &Handle) -> Self {
        Self(match screen {
            Screen::Site => format!("/{handle}"),
            Screen::Understood => format!("/{handle}/understood"),
            Screen::Deploys => format!("/{handle}/deploys"),
            Screen::Settings => format!("/{handle}/settings"),
            Screen::Account => "/account".to_owned(),
        })
    }

    /// The path of the Account screen.
    pub fn account() -> Self {
        Self("/account".to_owned())
    }

    /// The path of the sign-in page, which a form posts to for a link.
    pub fn signin() -> Self {
        Self("/signin".to_owned())
    }

    /// The path of the page said after a link was asked for.
    pub fn sent() -> Self {
        Self("/signin/sent".to_owned())
    }

    /// The path of the page a sign-in link opens, which a form posts to use the link.
    pub fn login(token: &Token) -> Self {
        Self(format!("/login/{token}"))
    }

    /// The path a form posts to end the session it holds.
    pub fn logout() -> Self {
        Self("/logout".to_owned())
    }

    /// The path a form posts to `/account/<change>`: `orgs`, `members`, `roles` or `removals`.
    pub fn seat(change: &'static str) -> Self {
        Self(format!("/account/{change}"))
    }

    /// The path a form posts to end the session `id`.
    pub fn session(id: Id<SessionKind>) -> Self {
        Self(format!("/account/sessions/{id}"))
    }

    /// The path of one build's log.
    pub fn build(handle: &Handle, build: BuildId) -> Self {
        Self(format!("/{handle}/deploys/{build}"))
    }

    /// The path a form posts `action` to.
    pub fn act(handle: &Handle, action: &'static str) -> Self {
        Self(format!("/{handle}/{action}"))
    }

    /// The path of the site's export.
    pub fn export(handle: &Handle) -> Self {
        Self(format!("/{handle}/export"))
    }

    /// The path of the replay of one decision's ladder.
    pub fn why(handle: &Handle, asked: &Asked) -> Self {
        let mut text = format!("/{handle}/understood/why?code={}", asked.code);
        if let model::Subject::File(path) = &asked.subject {
            text.push_str("&on=");
            Percent(path.as_str()).encode(&mut text);
        }
        Self(text)
    }

    /// The path of the decisions to check.
    pub fn check(handle: &Handle) -> Self {
        Self(format!("/{handle}/understood?only=check"))
    }

    /// As text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Path {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl AsRef<str> for Path {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Text written into a query: everything but unreserved characters and `/` is `%xx`.
#[derive(Clone, Copy, Debug)]
struct Percent<'a>(&'a str);

impl Percent<'_> {
    fn encode(self, out: &mut String) {
        for byte in self.0.bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
                out.push(char::from(byte));
            } else {
                let _infallible = write!(out, "%{byte:02X}");
            }
        }
    }
}

impl From<&Form> for Only {
    fn from(form: &Form) -> Self {
        if form.value(Field::Only) == Some("check") { Self::Check } else { Self::Everything }
    }
}

impl unspliced::AsUrl for Path {
    /// The path as a link target. Every constructor writes a leading `/`, so no path has a
    /// scheme and `Url::checked` accepts it; a test holds every constructor to that.
    fn url(&self) -> unspliced::Url<'_> {
        unspliced::Url::checked(&self.0).unwrap_or(unspliced::Url::new("/"))
    }
}
