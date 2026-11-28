//! The store a Worker uses.

use std::fmt;

use edge_api::{Claimed, Claiming, Nonces};
use worker::{Method, ObjectNamespace, Request, RequestInit};

use crate::{Answer, Wire};

/// The object could not be reached or did not answer a claim.
#[derive(Debug)]
pub struct Unreached {
    /// What the platform or the answer said.
    pub source: worker::Error,
}

impl fmt::Display for Unreached {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the nonce object did not answer")
    }
}

impl std::error::Error for Unreached {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Reaches the object of a key through a namespace binding: one object per key, named by the key.
#[derive(Debug)]
pub struct Remote {
    namespace: ObjectNamespace,
}

impl Remote {
    /// The address an object is asked at. Its host is not used: the stub is the destination.
    const URL: &'static str = "https://nonce.invalid/claim";

    /// A store over `namespace`.
    pub const fn new(namespace: ObjectNamespace) -> Self {
        Self { namespace }
    }

    async fn ask(&self, claiming: Claiming) -> worker::Result<Answer> {
        let stub = self.namespace.get_by_name(&claiming.key.to_string())?;
        let body = serde_json::to_string(&Wire::from(claiming))
            .map_err(|error| worker::Error::RustError(error.to_string()))?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_body(Some(body.into()));
        let mut response = stub.fetch_with_request(Request::new_with_init(Self::URL, &init)?).await?;
        if response.status_code() != 200 {
            return Err(worker::Error::RustError(format!("the nonce object answered {}", response.status_code())));
        }
        response.json().await
    }
}

impl Nonces for Remote {
    type Error = Unreached;

    async fn claim(&self, claiming: Claiming) -> Result<Claimed, Unreached> {
        self.ask(claiming).await.map(Claimed::from).map_err(|source| Unreached { source })
    }
}
