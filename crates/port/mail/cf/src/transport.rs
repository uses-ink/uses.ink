//! The port the provider call goes through.

use crate::Request;

/// Delivers one request to a provider. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Worker's fetch futures are not Send")]
pub trait Transport {
    /// The transport's own error. It must not hold the request's body: that carries a link's token.
    type Error: std::error::Error + 'static;

    /// Sends `request`, or says why it was not accepted.
    async fn deliver(&self, request: &Request) -> Result<(), Self::Error>;
}
