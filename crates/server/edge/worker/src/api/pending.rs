//! The routes the hosted Worker does not answer yet, answered as what they are: a `501` with the
//! catalog's code and its fix, not a `404` that says nothing is there.

use edge_api::door::Enter;
use edge_api::view::Problem;
use edge_api::{Answers, HOOKS, Media, Method, Reply, Request, Status, Step};

/// A part of the product the Worker does not have yet (decision 0201).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Pending {
    /// A page of the dashboard.
    Dashboard,
    /// The sign-in by mailed link, and its sessions.
    SignIn,
    /// Passkeys and recovery codes.
    Passkeys,
    /// A build, which needs the builder.
    Build,
}

impl Pending {
    /// The code of the catalog.
    pub const CODE: &'static str = "worker.route.pending";

    /// The fix the catalog gives, which every answer carries.
    const FIX: &'static str = "use the signed API under `/v1` with `ink`; the dashboard, sign-in, passkeys and builds of the hosted Worker are planned (decision 0201)";

    /// What `request` asks for that the Worker does not have, if it does.
    pub fn of(request: &Request) -> Option<Self> {
        let path = request.path.as_str();
        if Enter::owns(path) || path == "/login" || path.starts_with("/login/") {
            Some(Self::SignIn)
        } else if Step::owns(path) {
            Some(Self::Passkeys)
        } else if request.method == Method::Post && Self::builds(path) {
            Some(Self::Build)
        } else if !path.starts_with("/v1/") && !path.starts_with(HOOKS) {
            Some(Self::Dashboard)
        } else {
            None
        }
    }

    /// Whether `path` is the route that asks for a build: `/v1/sites/<id>/builds`.
    fn builds(path: &str) -> bool {
        let mut parts = path.trim_end_matches('/').split('/');
        matches!(
            (parts.next(), parts.next(), parts.next(), parts.next(), parts.next(), parts.next()),
            (Some(""), Some("v1"), Some("sites"), Some(_), Some("builds"), None)
        )
    }

    /// What it is, in the words of the answer.
    pub const fn what(self) -> &'static str {
        match self {
            Self::Dashboard => "the dashboard",
            Self::SignIn => "sign-in by link and its sessions",
            Self::Passkeys => "passkeys and recovery codes",
            Self::Build => "a build of a site from its source",
        }
    }

    /// The answer: `501`, the code, what is missing and what to do.
    pub fn reply(self) -> Reply {
        let problem = Problem {
            problem: Self::CODE,
            message: format!("the hosted Worker does not answer this yet: {}; {}", self.what(), Self::FIX),
            causes: Vec::new(),
            fields: std::collections::BTreeMap::from([("reason", self.what().to_owned())]),
        };
        Reply::new(Status::NotImplemented, Media::Json, serde_json::to_vec(&problem).unwrap_or_default())
    }
}

/// An answerer with the pending routes in front of it.
#[derive(Debug)]
pub struct Gate<A> {
    inner: A,
}

impl<A> Gate<A> {
    /// `inner` answers everything that is not pending.
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }

    /// The answerer behind the gate.
    pub const fn inner(&self) -> &A {
        &self.inner
    }
}

impl<A: Answers> Answers for Gate<A> {
    async fn answer(&self, request: &Request) -> Reply {
        match Pending::of(request) {
            Some(pending) => {
                logging::info!(name: logging::event::REQUEST_REFUSED, route = pending.what());
                pending.reply()
            }
            None => self.inner.answer(request).await,
        }
    }
}
