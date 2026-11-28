//! The watch's head reader, over the recorded advertisements of three forges.

use watch_worker::{Advertisement, Case, Fetch, Fetching, Forge, Get};
use worker::{Env, Result};

/// The watch cases.
pub(crate) struct Tips<'a> {
    pub(crate) env: &'a Env,
}

impl Tips<'_> {
    /// The case beside the suite: the platform's fetch reads an advertisement the rig serves.
    const FETCHED: &'static str = "fetches_an_advertisement_through_the_platform";

    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Case::ALL.iter().map(|case| case.name()).collect();
        names.push(Self::FETCHED);
        names
    }

    /// The platform's fetch gets the recorded GitHub body from the rig's own route and it reads as
    /// the same advertisement as the bytes do.
    async fn fetched(&self) -> Result<()> {
        let origin = self.env.var("SELF")?.to_string();
        let get = Get { url: format!("{origin}/__fixture/github"), basic: None };
        let reply = Fetching.get(&get).await.map_err(|error| worker::Error::RustError(error.to_string()))?;
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, Forge::Github.body());
        let read = Advertisement::parse(&reply.body).map_err(|error| worker::Error::RustError(error.to_string()))?;
        assert_eq!(read.default_branch(), Some("master"));
        Ok(())
    }

    /// Runs the case; false if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<bool> {
        if name == Self::FETCHED {
            self.fetched().await?;
        } else if let Some(case) = Case::named(name) {
            case.run().await;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
