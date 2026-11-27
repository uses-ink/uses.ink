//! The port itself.

use crate::Message;

/// Sends a message. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Workers email binding's futures are not Send")]
pub trait Mailer {
    /// The adapter's own error.
    type Error: std::error::Error + 'static;

    /// Sends `message`, or says why it was not sent.
    async fn send(&self, message: &Message) -> Result<(), Self::Error>;
}

impl<M: Mailer + ?Sized> Mailer for &M {
    type Error = M::Error;

    async fn send(&self, message: &Message) -> Result<(), Self::Error> {
        (**self).send(message).await
    }
}
