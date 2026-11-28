//! The dashboard of a self-host, mounted on the API's own host (decision 0090).
//!
//! `/v1/...` is the API and everything else is a page of `web`. A session cookie belongs to the
//! host that set it, a passkey belongs to a relying party id, and the dashboard's one script
//! fetches `/v1/passkeys/...` with the cookie, so the pages and the routes they call must share
//! a host: nothing else lets the cookie, the passkey and the Origin check agree.
//!
//! [`Hosted`] puts the pages in front of the API's answerer. [`Browser`] is one request's
//! [`web::Screens`]: sites, builds and deploys from the control plane, and everything
//! about who is signed in from the API's own routes, called as the browser that asked.

mod browser;
mod replay;
mod said;

use std::cell::RefCell;

use authority_dns::Chosen;
use authority_repo::{Repos, Wire};
use control::Control;
use edge_api::{Answers, Cookie, Header, Limits, Media, Method, Origins, Payload, Reply, Request, Status};
use web::{Body, Cause, Dashboard, Failure, Form, Frame};

use crate::selfhost::dash::browser::{Browser, Idle};
use crate::selfhost::dash::said::{Elsewhere, Said};
use crate::selfhost::product::Ports;

/// What the pages need beyond the API: the control plane, which origins may post, and the limits
/// of what a page may hold.
#[derive(Debug)]
pub(crate) struct Pages {
    control: Control<Ports>,
    origins: Origins,
    resolver: Chosen,
    repos: Option<Repos<Wire>>,
    limits: Limits,
}

impl Pages {
    /// The pages over `control`, which post only from `origins`, read proof records through
    /// `resolver` and claim repositories through `repos` when forges are on, within `limits`.
    pub(crate) fn new(control: Control<Ports>, trust: (Origins, Chosen, Option<Repos<Wire>>), limits: Limits) -> Self {
        Self { control, origins: trust.0, resolver: trust.1, repos: trust.2, limits }
    }
}

/// The pages of the dashboard in front of the API's answerer.
#[derive(Debug)]
pub(crate) struct Hosted<A> {
    api: A,
    pages: Pages,
}

impl<A> Hosted<A> {
    /// `pages` for every path that is not the API's, which `api` answers.
    pub(crate) const fn new(api: A, pages: Pages) -> Self {
        Self { api, pages }
    }

    /// Whether `path` is one of the API's routes.
    fn owns(path: &str) -> bool {
        path == "/v1" || path.starts_with("/v1/")
    }

    /// The dashboard's method for the API's, when a page takes it.
    const fn method(method: Method) -> Option<web::Method> {
        match method {
            Method::Get => Some(web::Method::Get),
            Method::Post => Some(web::Method::Post),
            _ => None,
        }
    }
}

impl<A: Answers> Answers for Hosted<A> {
    async fn answer(&self, request: &Request) -> Reply {
        if Self::owns(&request.path) { self.api.answer(request).await } else { self.page(request).await }
    }
}

impl<A: Answers> Hosted<A> {
    /// One page: the origin of a post checked, who is signed in asked of the API, the dashboard
    /// asked for its reply, and the cookie the API set on the way passed to the browser.
    #[expect(clippy::too_many_lines, reason = "one sequence of refusals, each a few lines")]
    async fn page(&self, request: &Request) -> Reply {
        let Some(method) = Self::method(request.method) else {
            let mut reply = Reply::empty(Status::MethodNotAllowed);
            reply.allow = vec![Method::Get, Method::Post];
            return reply;
        };
        let nowhere = Frame { site: None, here: None };
        if method == web::Method::Post && !self.pages.origins.admits(request) {
            let failure = Failure::Auth {
                status: web::Status::Forbidden,
                problem: "auth.origin.refused".to_owned(),
                source: Cause::of(&Elsewhere),
            };
            return Self::convert(Dashboard::<Browser<'_, A>>::failed(nowhere, failure), None).await;
        }
        let body = match &request.body {
            Payload::Whole(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            other => {
                let failure = Failure::Auth {
                    status: web::Status::BadRequest,
                    problem: "body.too-large".to_owned(),
                    source: Cause::of(&Said::over(other)),
                };
                return Self::convert(Dashboard::<Browser<'_, A>>::failed(nowhere, failure), None).await;
            }
        };
        // a face or a stroke is the same for everyone: no session is asked for and no cookie rides with it
        let asset = request.path.strip_prefix('/').and_then(web::Asset::named).filter(|_| method == web::Method::Get);
        if let Some(asset) = asset {
            let reply = web::Reply::<Idle> { status: web::Status::Ok, location: None, body: Body::Asset(asset) };
            return Self::convert(reply, None).await;
        }
        let browser = Browser { pages: &self.pages, api: &self.api, request, cookie: RefCell::new(None) };
        let who = match browser.who().await {
            Ok(who) => who,
            Err(failure) => {
                return Self::convert(Dashboard::<Browser<'_, A>>::failed(nowhere, failure), browser.cookie.take())
                    .await;
            }
        };
        let asked = web::Request {
            method,
            path: &request.path,
            query: request.query.as_deref().map_or_else(Form::default, Form::from),
            body: Form::from(body.as_str()),
        };
        let dashboard = Dashboard(browser);
        let reply = dashboard.respond(who, &asked).await;
        Self::convert(reply, dashboard.0.cookie.take()).await
    }

    /// A face, the mark or a stroke as the API sends it: kept a day, since it never changes while
    /// the process runs.
    fn served(asset: web::Asset, headers: &mut Vec<Header>) -> (Media, Vec<u8>) {
        headers.retain(|header| header.name != "cache-control");
        headers.push(Header { name: "cache-control", value: "public, max-age=86400".to_owned() });
        let media = match asset.medium() {
            web::Medium::Font => Media::Font,
            web::Medium::Style => Media::Css,
            _ => Media::Svg,
        };
        (media, asset.bytes().into_owned())
    }

    /// The dashboard's reply as the API's: its status, its headers, its page, and the cookie.
    async fn convert(reply: web::Reply<Idle>, cookie: Option<Cookie>) -> Reply {
        let status = Said::status_of(reply.status);
        let mut headers: Vec<Header> = web::Reply::<Idle>::HEADERS
            .iter()
            .map(|(name, value)| Header { name, value: (*value).to_owned() })
            .collect();
        if let Some(path) = &reply.location {
            headers.push(Header { name: "location", value: path.to_string() });
        }
        let (media, body) = match reply.body {
            Body::Page(text) => (Media::Html, text.into_bytes()),
            Body::Script(text) => (Media::Script, text.as_bytes().to_vec()),
            Body::Asset(asset) => Self::served(asset, &mut headers),
            Body::Download(file) => {
                headers.push(Header { name: "content-disposition", value: format!("attachment; filename=\"{file}\"") });
                (Media::Tar, file.bytes)
            }
            Body::Streamed(mut stream) => {
                // a self-host stores a build's log when it ends, so nothing streams: a stream is
                // read to its end and sent whole
                let mut page = String::new();
                while let Some(chunk) = stream.chunk().await {
                    match chunk {
                        Ok(text) => page.push_str(&text),
                        Err(failure) => {
                            return Box::pin(Self::convert(
                                Dashboard::<Browser<'_, A>>::failed(Frame { site: None, here: None }, failure),
                                cookie,
                            ))
                            .await;
                        }
                    }
                }
                (Media::Html, page.into_bytes())
            }
            _ => (Media::Empty, Vec::new()),
        };
        let mut sent = Reply::new(status, media, body);
        sent.headers = headers;
        sent.cookie = cookie;
        sent
    }
}
