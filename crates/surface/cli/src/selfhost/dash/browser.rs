//! The dashboard's [`Screens`] over the control plane, for the one request that asked.
//!
//! Sites, builds, deploys and the changes a form asks for go to the control plane. Everything
//! about who is signed in goes through the API's own routes, called as the browser that asked
//! (its cookie, its origin and its source address ride along): the dashboard has no second copy
//! of the sign-in rules, so a rate limit, an origin check and a revoked session are the ones the
//! API already has.

use std::cell::RefCell;
use std::time::Duration;

use build::{BuildId, Event, Status as Built, Trigger};
use clock::{Clock as _, Draw as _, System};
use control::{AccountKind, Handle, Id, JobState, NewJob, Refusal, SessionKind};
use edge_api::{Answers, Cookie, Exports, Method, Payload, Reply, Request, Status};
use jiff::Timestamp;
use model::ExternalUrl;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use web::view;
use web::{Action, Asked, Cause, Download, Failure, Follow, Logged, Only, Part, Screens, Token, Unrouted};

use crate::selfhost::dash::Pages;
use crate::selfhost::dash::replay::Replay;
use crate::selfhost::dash::said::Said;

/// Where a running build's events would come from. A self-host stores a log when the build ends,
/// so a running build's page is the stored one and refreshes, and this is never asked.
#[derive(Debug)]
pub(crate) struct Idle;

impl Follow for Idle {
    fn next(&mut self) -> impl std::future::Future<Output = Result<Option<Event>, Failure>> + Send {
        std::future::ready(Ok(None))
    }
}

/// The session a probe of `GET /v1/session` names.
#[derive(Debug, Deserialize)]
struct Shown {
    account: String,
}

/// A session as `GET /v1/sessions` lists it.
#[derive(Debug, Deserialize)]
struct Listed {
    id: String,
    created: Timestamp,
    seen: Timestamp,
    current: bool,
}

/// A passkey as `GET /v1/passkeys` lists it.
#[derive(Debug, Deserialize)]
struct Passkey {
    id: String,
    label: String,
    created: Timestamp,
    used: Option<String>,
    synced: bool,
}

/// What `GET /v1/recovery/codes` counts.
#[derive(Debug, Deserialize)]
struct Left {
    remaining: u64,
}

/// What a site shows: what it serves, and its builds, newest first.
#[derive(Debug)]
struct Ledger {
    serving: view::Serving,
    attempts: Vec<view::Attempt>,
    jobs: Vec<control::Job>,
}

/// One request's screens.
#[derive(Debug)]
pub(crate) struct Browser<'a, A> {
    pub(super) pages: &'a Pages,
    pub(super) api: &'a A,
    pub(super) request: &'a Request,
    /// The cookie the API set while answering for this request: a new session, or an ended one.
    pub(super) cookie: RefCell<Option<Cookie>>,
}

impl<A: Answers> Browser<'_, A> {
    /// How many builds the Site screen lists; Deploys has the rest.
    const RECENT: usize = 5;

    /// A call of the API's own routes as the browser that asked.
    async fn call(&self, method: Method, path: &str, body: Vec<u8>) -> Reply {
        let asked = Request {
            method,
            path: path.to_owned(),
            query: None,
            authorization: None,
            cookie: self.request.cookie.clone(),
            origin: self.request.origin.clone(),
            site: self.request.site.clone(),
            source: self.request.source,
            body: Payload::Whole(body),
        };
        self.api.answer(&asked).await
    }

    /// Keeps the cookie a reply sets, to hand to the browser.
    fn keep(&self, cookie: Option<Cookie>) {
        if cookie.is_some() {
            *self.cookie.borrow_mut() = cookie;
        }
    }

    /// A read of the API's own routes, as the JSON `T`.
    async fn json<T: DeserializeOwned>(&self, path: &str) -> Result<T, Failure> {
        let reply = self.call(Method::Get, path, Vec::new()).await;
        if reply.status != Status::Ok {
            return Err(Said::failure(&reply));
        }
        serde_json::from_slice(&reply.body).map_err(|source| Said::unread(&source))
    }

    /// Who the request is from: the account of its session, or nobody. A cookie the server does
    /// not know is cleared by the reply.
    pub(super) async fn who(&self) -> Result<Option<Id<AccountKind>>, Failure> {
        let reply = self.call(Method::Get, "/v1/session", Vec::new()).await;
        match reply.status {
            Status::Ok => {
                let shown: Shown = serde_json::from_slice(&reply.body).map_err(|source| Said::unread(&source))?;
                let account = shown.account.parse().map_err(|source| Said::unread(&source))?;
                Ok(Some(account))
            }
            Status::Unauthorized => {
                self.keep(reply.cookie);
                Ok(None)
            }
            _ => Err(Said::failure(&reply)),
        }
    }

    /// The site of `handle`, when it is `who`'s.
    async fn owned(&self, who: Id<AccountKind>, handle: &Handle) -> Result<control::Site, Failure> {
        let site = self.pages.control.by_handle(handle).await?;
        let site = site.ok_or_else(|| Failure::NoSite { handle: handle.clone() })?;
        if site.account == who { Ok(site) } else { Err(Failure::Foreign { handle: handle.clone() }) }
    }

    /// The address the site is served at.
    fn address(&self, handle: &Handle) -> Result<ExternalUrl, Failure> {
        let url = self.pages.control.origin().site(handle);
        url.map_err(|source| Failure::Store { part: Part::Records, source: Cause::of(&source) })
    }

    /// What the site serves and its builds.
    async fn ledger(&self, site: &control::Site) -> Result<Ledger, Failure> {
        let (control, rows) = (&self.pages.control, self.pages.limits.rows);
        let deploys = control.deploys(site.id, rows).await?;
        let serving = match control.live(site.id).await? {
            None => view::Serving::Nothing,
            Some(live) => {
                let held = match deploys.iter().find(|deploy| deploy.id == live.deploy) {
                    Some(deploy) => deploy.clone(),
                    // the live deploy is older than the list: ask for them all
                    None => control
                        .deploys(site.id, u32::MAX)
                        .await?
                        .into_iter()
                        .find(|deploy| deploy.id == live.deploy)
                        .ok_or(Refusal::NoDeploy { site: site.id, deploy: live.deploy })
                        .map_err(|refusal| Failure::Refused { refusal })?,
                };
                let pointer =
                    view::Pointer { deploy: live.deploy, since: live.since, files: held.files, bytes: held.bytes };
                if live.paused { view::Serving::Paused(pointer) } else { view::Serving::Live(pointer) }
            }
        };
        // the builds of branches make previews, not deploys: they are not the site's history
        let jobs: Vec<control::Job> =
            control.jobs(site.id, rows).await?.into_iter().filter(|job| job.pin.is_none()).collect();
        let made = |job: &control::Job| {
            let found = job.deploy.and_then(|id| deploys.iter().find(|deploy| deploy.id == id));
            view::Attempt::of(job, found.map(view::Deploy::from))
        };
        let attempts = jobs.iter().map(made).collect();
        Ok(Ledger { serving, attempts, jobs })
    }

    /// A build's stored log, when it wrote one.
    async fn replay(&self, site: &control::Site, job: &control::Job) -> Result<Option<Replay>, Failure> {
        match self.pages.control.build_log(site.id, job.id).await? {
            Some(bytes) => Ok(Some(Replay::read(&bytes)?)),
            None => Ok(None),
        }
    }

    /// How a build that ended ended.
    fn outcome(job: &control::Job, replay: Option<&Replay>) -> view::Outcome {
        let status = match job.state {
            JobState::Succeeded => Built::Succeeded,
            JobState::Refused => Built::Refused,
            _ => Built::Failed,
        };
        let waited = job.finished.and_then(|done| Duration::try_from(done.duration_since(job.requested)).ok());
        let wall = replay.and_then(Replay::wall).or(waited).unwrap_or_default();
        view::Outcome { status, deploy: job.deploy, wall }
    }

    /// The custom domains of a site: the names it serves, then the ones it has still to prove with
    /// the record each needs.
    async fn domains(&self, site: Id<control::SiteKind>) -> Result<Vec<view::Custom>, Failure> {
        let listed = self.pages.control.domains(site).await?;
        let proven = listed
            .proven
            .iter()
            .map(|domain| view::Custom { name: domain.name.to_string(), standing: view::Standing::Proven(domain.at) });
        let pending = listed.pending.iter().map(|claim| {
            let challenge = authority_dns::Challenge::of(claim);
            view::Custom {
                name: claim.name.to_string(),
                standing: view::Standing::Pending { host: challenge.host.to_string(), value: challenge.value },
            }
        });
        Ok(proven.chain(pending).collect())
    }

    /// Reads the record of `name` and makes it serve `site`, or says why it does not.
    async fn prove(&self, site: Id<control::SiteKind>, name: &control::DomainName) -> Result<(), Failure> {
        let dns = authority_dns::Dns::new(self.pages.resolver.clone());
        match dns.prove(&self.pages.control, (site, name.clone()), System.now()).await {
            Ok(_) => Ok(()),
            Err(authority_dns::Error::Lookup(error)) => {
                Err(Failure::Lookup { name: name.to_string(), source: Cause::of(&error) })
            }
            Err(authority_dns::Error::Control(error)) => Err(Failure::from(error)),
        }
    }

    /// Sixteen bytes nobody has been given before.
    fn draw() -> Result<[u8; 16], Failure> {
        System.bytes().map_err(|source| Failure::Store { part: Part::Records, source: Cause::of(&source) })
    }
}

impl<A: Answers> Screens for Browser<'_, A> {
    type Follow = Idle;

    fn limits(&self) -> control::Limits {
        *self.pages.control.limits()
    }

    async fn site(&self, who: Id<AccountKind>, handle: &Handle) -> Result<view::Site, Failure> {
        let site = self.owned(who, handle).await?;
        let ledger = self.ledger(&site).await?;
        let mut page = view::Site::new(handle.clone(), self.address(handle)?, ledger.serving.clone());
        page.recent = ledger.attempts.iter().take(Self::RECENT).cloned().collect();
        if let Some(job) = ledger.jobs.iter().find(|job| job.state.ended())
            && let Some(replay) = self.replay(&site, job).await?
        {
            page.observed = replay.observed()?;
            page.flagged = replay.flagged();
        }
        Ok(page)
    }

    async fn understood(&self, who: Id<AccountKind>, handle: &Handle, only: Only) -> Result<view::Understood, Failure> {
        let site = self.owned(who, handle).await?;
        let ledger = self.ledger(&site).await?;
        let mut page = view::Understood::new(handle.clone(), site.source.clone(), Vec::new());
        page.only = only;
        if let Some(job) = ledger.jobs.iter().find(|job| job.state == JobState::Succeeded)
            && let Some(replay) = self.replay(&site, job).await?
        {
            page.decisions = replay.decisions()?;
            page.observed = replay.observed()?;
            page.deploy = job.deploy;
        }
        Ok(page)
    }

    #[expect(clippy::unused_async_trait_impl, reason = "the trait is async; a self-host keeps no replay to ask")]
    async fn why(&self, _who: Id<AccountKind>, _handle: &Handle, _asked: &Asked) -> Result<view::Why, Failure> {
        Err(Failure::NoReplay)
    }

    async fn deploys(&self, who: Id<AccountKind>, handle: &Handle) -> Result<view::Deploys, Failure> {
        let site = self.owned(who, handle).await?;
        let ledger = self.ledger(&site).await?;
        let control = &self.pages.control;
        let mut branches = Vec::new();
        for preview in control.previews(site.id, System.now()).await? {
            let url = control
                .origin()
                .previewed(&preview.slug, handle)
                .map_err(|source| Failure::Store { part: Part::Records, source: Cause::of(&source) })?;
            branches.push(view::Branch {
                label: preview.slug.to_string(),
                url: url.to_string(),
                files: preview.files,
                ends: preview.expires,
            });
        }
        Ok(view::Deploys::new(handle.clone(), ledger.serving, ledger.attempts).with_previews(branches))
    }

    async fn settings(&self, who: Id<AccountKind>, handle: &Handle) -> Result<view::Settings, Failure> {
        let site = self.owned(who, handle).await?;
        let ledger = self.ledger(&site).await?;
        let mut page = view::Settings::new(handle.clone(), self.address(handle)?, site.source.clone(), ledger.serving);
        let held = self.pages.control.credentials(site.id).await?;
        page.credentials = held
            .iter()
            .map(|credential| view::Held { purpose: credential.purpose, created: credential.created })
            .collect();
        page.domains = self.domains(site.id).await?;
        Ok(page)
    }

    async fn account(&self, who: Id<AccountKind>) -> Result<view::Account, Failure> {
        let control = &self.pages.control;
        let created = control.account(who).await?.created;
        let owned = control.sites(who).await?;
        let sites = owned
            .iter()
            .map(|site| view::Owned { handle: site.handle.clone(), state: site.state, created: site.created })
            .collect();
        let mut sessions = Vec::new();
        for listed in self.json::<Vec<Listed>>("/v1/sessions").await? {
            let id: Id<SessionKind> = listed.id.parse().map_err(|source| Said::unread(&source))?;
            sessions.push(view::Seen { id, created: listed.created, seen: listed.seen, current: listed.current });
        }
        let passkeys = self
            .json::<Vec<Passkey>>("/v1/passkeys")
            .await?
            .into_iter()
            .map(|key| view::Kept {
                id: key.id,
                label: key.label,
                created: key.created,
                used: key.used,
                synced: key.synced,
            })
            .collect();
        let left: Left = self.json("/v1/recovery/codes").await?;
        Ok(view::Account::new(created, sites)
            .with_passkeys(passkeys)
            .with_recovery(left.remaining)
            .with_sessions(sessions))
    }

    async fn end_session(&self, _who: Id<AccountKind>, id: Id<SessionKind>) -> Result<(), Failure> {
        let reply = self.call(Method::Delete, &format!("/v1/sessions/{id}"), Vec::new()).await;
        match reply.status {
            Status::Ok => {
                self.keep(reply.cookie);
                Ok(())
            }
            Status::NotFound => Err(Failure::Unrouted { source: Unrouted::Path }),
            _ => Err(Said::failure(&reply)),
        }
    }

    async fn ask(&self, address: &str) -> Result<(), Failure> {
        let body = serde_json::json!({ "address": address }).to_string().into_bytes();
        let reply = self.call(Method::Post, "/v1/login", body).await;
        if reply.status == Status::Accepted { Ok(()) } else { Err(Said::failure(&reply)) }
    }

    async fn enter(&self, token: &Token, address: Option<&str>) -> Result<(), Failure> {
        let body =
            address.map_or_else(Vec::new, |address| serde_json::json!({ "address": address }).to_string().into_bytes());
        let reply = self.call(Method::Post, &format!("/v1/login/{token}"), body).await;
        if reply.status == Status::Ok {
            self.keep(reply.cookie);
            Ok(())
        } else {
            Err(Said::failure(&reply))
        }
    }

    async fn sign_out(&self, _who: Id<AccountKind>) -> Result<(), Failure> {
        let reply = self.call(Method::Delete, "/v1/session", Vec::new()).await;
        if reply.status == Status::Ok {
            self.keep(reply.cookie);
            Ok(())
        } else {
            Err(Said::failure(&reply))
        }
    }

    async fn log(&self, who: Id<AccountKind>, handle: &Handle, build: BuildId) -> Result<Logged<Idle>, Failure> {
        let site = self.owned(who, handle).await?;
        let job = self.pages.control.job(build).await?;
        let job = job.filter(|job| job.site == site.id).ok_or(Failure::NoBuild { build })?;
        let replay = self.replay(&site, &job).await?;
        let lines = replay.as_ref().map_or_else(Vec::new, Replay::recorded);
        let ended = job.state.ended().then(|| view::Ended::Finished(Self::outcome(&job, replay.as_ref())));
        Ok(Logged::Stored(view::Stored::new(handle.clone(), build, lines, ended)))
    }

    async fn act(&self, who: Id<AccountKind>, handle: &Handle, action: &Action) -> Result<(), Failure> {
        let site = self.owned(who, handle).await?;
        let (control, now) = (&self.pages.control, System.now());
        match action {
            Action::Build => {
                if site.source == build::Source::Upload {
                    return Err(Failure::Upload { handle: handle.clone() });
                }
                let id = BuildId::from_bytes(Self::draw()?);
                control
                    .request_build(NewJob { id, site: site.id, trigger: Trigger::Manual, dedupe: None, at: now })
                    .await?;
            }
            Action::Rollback { deploy } => {
                control.rollback(site.id, *deploy, now).await?;
            }
            Action::Resume => {
                control.resume(site.id, now).await?;
            }
            Action::Remove { confirmed } => {
                if *confirmed != site.handle {
                    return Err(Failure::Confirm { expected: site.handle });
                }
                control.remove_site(site.id, now).await?;
            }
            Action::Claim { name } => {
                let token = control::Token::from_bytes(Self::draw()?);
                control.claim_domain(control::NewClaim { site: site.id, name: name.clone(), token, at: now }).await?;
            }
            Action::Prove { name } => self.prove(site.id, name).await?,
            Action::Unclaim { name } => control.remove_domain(site.id, name, now).await?,
            _ => return Err(Failure::Unrouted { source: Unrouted::Path }),
        }
        Ok(())
    }

    async fn export(&self, who: Id<AccountKind>, handle: &Handle) -> Result<Download, Failure> {
        let site = self.owned(who, handle).await?;
        let mut sink = Exports::new(self.pages.limits.export);
        match self.pages.control.export(site.id, None, &mut sink).await {
            Ok(_) => Ok(Download { of: handle.clone(), bytes: sink.finish() }),
            Err(control::Failed::Control(error)) => Err(Failure::from(error)),
            Err(other) => Err(Failure::Store { part: Part::Records, source: Cause::of(&other) }),
        }
    }
}
