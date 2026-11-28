//! What the provider is asked to send.

use std::fmt;

use mail::{Address, Message};

/// One message as a provider's API takes it: plain text, one recipient.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    /// The sender.
    pub from: Address,
    /// The recipient.
    pub to: Address,
    /// The subject line.
    pub subject: String,
    /// The plain-text body, which holds a link's token: it is not in `Debug`.
    pub text: String,
}

impl Request {
    /// The request that sends `message` from `from`.
    pub fn new(from: &Address, message: &Message) -> Self {
        Self {
            from: from.clone(),
            to: message.to.clone(),
            subject: message.template.subject().to_owned(),
            text: message.template.to_string(),
        }
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Request")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("subject", &self.subject)
            .field("text", &format_args!("<{} bytes>", self.text.len()))
            .finish()
    }
}
