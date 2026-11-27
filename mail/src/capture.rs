//! A mailer that keeps what it is given.

use std::convert::Infallible;
use std::sync::{Mutex, PoisonError};

use crate::{Mailer, Message};

/// Keeps every message in order, so a test reads what would have been sent.
#[derive(Debug, Default)]
pub struct Capture(Mutex<Vec<Message>>);

impl Capture {
    /// A mailer that has sent nothing.
    pub const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    /// Everything sent so far.
    pub fn sent(&self) -> Vec<Message> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl Mailer for Capture {
    type Error = Infallible;

    fn send(&self, message: &Message) -> impl std::future::Future<Output = Result<(), Infallible>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push(message.clone());
        std::future::ready(Ok(()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Base, Template};

    #[test]
    fn keeps_messages_in_order() {
        let capture = Capture::new();
        let base: Base = "https://x.example/login".parse().unwrap();
        let valid = std::time::Duration::from_mins(15);
        for who in ["a@x.org", "b@x.org"] {
            let message =
                Message { to: who.parse().unwrap(), template: Template::Login { link: base.link("t"), valid } };
            pollster::block_on(capture.send(&message)).unwrap();
        }
        let sent = capture.sent();
        assert_eq!(sent.iter().map(|m| m.to.as_str()).collect::<Vec<_>>(), ["a@x.org", "b@x.org"]);
        assert!(sent[0].template.to_string().contains("15 minutes"));
    }
}
