//! One server for the control plane's sites and a folder's.

use std::error::Error;
use std::fmt;

use edge::{Hostname, Resolved, Sites};

/// Which of two adapters failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum Either<A, B> {
    /// The first adapter.
    First(A),
    /// The second.
    Then(B),
}

impl<A, B> fmt::Display for Either<A, B> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::First(_) => out.write_str("the first of two lookups failed"),
            Self::Then(_) => out.write_str("the second of two lookups failed"),
        }
    }
}

impl<A: Error + 'static, B: Error + 'static> Error for Either<A, B> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::First(source) => Some(source),
            Self::Then(source) => Some(source),
        }
    }
}

/// Asks `first`, when there is one, and then `then` for a host `first` has no site for. A
/// failure of `first` is the answer: a control plane that is down is not a reason to serve a
/// folder's page for its host.
#[derive(Debug)]
pub struct Fallback<A, B> {
    first: Option<A>,
    then: B,
}

impl<A, B> Fallback<A, B> {
    /// `first`, then `then`.
    pub const fn new(first: Option<A>, then: B) -> Self {
        Self { first, then }
    }
}

impl<A, B> Sites for Fallback<A, B>
where
    A: Sites,
    B: Sites<Scope = A::Scope>,
{
    type Scope = A::Scope;
    type Error = Either<A::Error, B::Error>;

    async fn resolve(&self, hostname: &Hostname) -> Result<Option<Resolved<A::Scope>>, Self::Error> {
        if let Some(first) = &self.first
            && let Some(found) = first.resolve(hostname).await.map_err(Either::First)?
        {
            return Ok(Some(found));
        }
        self.then.resolve(hostname).await.map_err(Either::Then)
    }

    async fn suspended(&self, hostname: &Hostname) -> Result<Option<edge::Notice>, Self::Error> {
        if let Some(first) = &self.first
            && let Some(notice) = first.suspended(hostname).await.map_err(Either::First)?
        {
            return Ok(Some(notice));
        }
        self.then.suspended(hostname).await.map_err(Either::Then)
    }
}
