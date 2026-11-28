//! One transport for the three forges.

use std::sync::Arc;

use authority_forge::{Request, Response, Transport};

/// A transport behind a pointer: cloning it shares the one underneath.
#[derive(Debug)]
pub struct Shared<T>(Arc<T>);

impl<T> Shared<T> {
    /// Shares `transport`.
    pub fn new(transport: T) -> Self {
        Self(Arc::new(transport))
    }

    /// The transport underneath, for a test to read what a fake was asked.
    pub fn inner(&self) -> &T {
        &self.0
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: Transport> Transport for Shared<T> {
    type Error = T::Error;

    fn send(&self, request: Request) -> impl Future<Output = Result<Response, T::Error>> {
        self.0.send(request)
    }
}
