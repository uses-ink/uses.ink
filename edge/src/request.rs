//! A request as an adapter hands it over, and the checked form the edge works with.

use std::fmt;

use crate::condition::Conditions;
use crate::encoding::Accepted;
use crate::head::{Head, Singular};
use crate::host::{Authority, Hostname, HostnameError};
use crate::limits::Limits;
use crate::range::Ranges;
use crate::refusal::Refusal;
use crate::tag::{Matching, Validator};
use crate::target::Target;

/// The methods the edge serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `HEAD`: a `GET` without the body.
    Head,
}

impl Method {
    /// What `Allow` lists.
    pub const ALLOWED: &'static str = "GET, HEAD";
}

impl fmt::Display for Method {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
        })
    }
}

/// How the client reached the edge. HSTS is only meaningful, and only sent, over TLS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// Plain HTTP.
    Plain,
    /// TLS.
    Tls,
}

/// A request exactly as received: nothing in it is trusted until [`Request::ask`].
#[derive(Clone, Debug)]
pub struct Request<'a> {
    method: &'a str,
    target: &'a str,
    pub(crate) scheme: Scheme,
    headers: Vec<(&'a [u8], &'a [u8])>,
}

impl<'a> Request<'a> {
    /// A request with no headers yet.
    pub fn new(method: &'a str, target: &'a str, scheme: Scheme) -> Self {
        Self { method, target, scheme, headers: Vec::new() }
    }
}

impl<'a> Extend<(&'a [u8], &'a [u8])> for Request<'a> {
    /// Adds header lines, name then value, in the order they arrived.
    fn extend<I: IntoIterator<Item = (&'a [u8], &'a [u8])>>(&mut self, lines: I) {
        self.headers.extend(lines);
    }
}

/// A request that passed every check: a method served, a host, a path that names no more than
/// one thing, and what the client said about its copy.
#[derive(Clone, Debug)]
pub struct Asked {
    pub(crate) method: Method,
    pub(crate) hostname: Hostname,
    pub(crate) target: Target,
    pub(crate) conditions: Conditions,
    pub(crate) scheme: Scheme,
}

impl Request<'_> {
    /// Checks the request against `limits`. The first thing wrong is the refusal: the method,
    /// then the headers, then the host, the target and the conditions.
    pub fn ask(&self, limits: &Limits) -> Result<Asked, Refusal> {
        let method = match self.method {
            "GET" => Method::Get,
            "HEAD" => Method::Head,
            other => return Err(Refusal::Method { method: other.to_owned() }),
        };
        let head = Head(&self.headers);
        head.check(limits)?;
        head.bodiless()?;
        let target = Target::parse(self.target, limits)?;
        let header = head.one(Singular::Host)?.ok_or(HostnameError::Missing)?;
        let Authority { hostname, .. } = Authority::try_from(header)?;
        if let Some(named) = target.authority() {
            if named.hostname != hostname {
                return Err(Refusal::Authority { header: hostname, target: named.hostname.clone() });
            }
        }
        let conditions = Conditions {
            none_match: head.one(Singular::IfNoneMatch)?.map(|value| Matching::read(value, limits.tags)).transpose()?,
            range: head.one(Singular::Range)?.map(Ranges::read).transpose()?,
            if_range: head.one(Singular::IfRange)?.map(Validator::read).transpose()?,
            accept: head.one(Singular::AcceptEncoding)?.map(Accepted::read).transpose()?.unwrap_or_default(),
        };
        Ok(Asked { method, hostname, target, conditions, scheme: self.scheme })
    }
}

impl Asked {
    /// The host the request is for.
    pub fn hostname(&self) -> &Hostname {
        &self.hostname
    }

    /// The target, decoded and normalised.
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// The method.
    pub fn method(&self) -> Method {
        self.method
    }
}
