//! The adapter.

use mail::{Address, Mailer, Message};

use crate::{Error, Request, Transport};

/// Sends messages through a transport, from one address.
#[derive(Debug)]
pub struct Cf<T> {
    from: Address,
    transport: T,
}

impl<T: Transport> Cf<T> {
    /// A mailer that sends from `from` through `transport`.
    pub const fn new(from: Address, transport: T) -> Self {
        Self { from, transport }
    }

    /// The transport, for a test that reads what it kept.
    pub const fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T: Transport> Mailer for Cf<T> {
    type Error = Error<T::Error>;

    async fn send(&self, message: &Message) -> Result<(), Self::Error> {
        let request = Request::new(&self.from, message);
        self.transport.deliver(&request).await.map_err(|source| Error { to: message.to.clone(), source })
    }
}
