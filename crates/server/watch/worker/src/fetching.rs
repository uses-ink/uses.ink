//! The Worker's fetch.

use std::fmt;

use worker::{Fetch as Outbound, Headers, Method, Request, RequestInit};

use crate::{Fetch, Get, Reply};

/// The platform's fetch failed: no answer, or a body that could not be read.
#[derive(Debug)]
pub struct Failed {
    /// What the platform said.
    pub source: worker::Error,
}

impl fmt::Display for Failed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the platform's fetch failed")
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// `fetch` as a Worker has it: redirects are followed, and the login goes in `Authorization`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fetching;

impl Fetching {
    /// What the request says it is.
    const AGENT: &'static str = "uses-ink-watch";

    async fn call(get: &Get) -> worker::Result<Reply> {
        let headers = Headers::new();
        headers.set("User-Agent", Self::AGENT)?;
        if let Some(basic) = &get.basic {
            headers.set("Authorization", basic.value())?;
        }
        let mut init = RequestInit::new();
        init.with_method(Method::Get).with_headers(headers);
        let mut response = Outbound::Request(Request::new_with_init(&get.url, &init)?).send().await?;
        Ok(Reply { status: response.status_code(), body: response.bytes().await? })
    }
}

impl Fetch for Fetching {
    type Error = Failed;

    async fn get(&self, get: &Get) -> Result<Reply, Failed> {
        Self::call(get).await.map_err(|source| Failed { source })
    }
}
