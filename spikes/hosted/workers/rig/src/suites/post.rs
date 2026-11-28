//! The mail port's suite, through the Cloudflare adapter over a fake provider.

use mail_cf::{Cf, Fake};
use mail_conformance::{Case, Delivered, Harness, Refusing};
use worker::Result;

/// The mail cases.
pub(crate) struct Post;

/// The adapter over the fake provider, read back from what the provider took.
struct Provided(Cf<Fake>);

impl Harness for Provided {
    type Mailer = Cf<Fake>;

    fn mailer(&self) -> &Cf<Fake> {
        &self.0
    }

    fn delivered(&self) -> Vec<Delivered> {
        self.0
            .transport()
            .taken()
            .into_iter()
            .map(|request| Delivered { to: request.to.to_string(), subject: request.subject, body: request.text })
            .collect()
    }

    fn refuse(&self, refusing: bool) -> Refusing {
        self.0.transport().down(refusing);
        Refusing::Told
    }
}

impl Post {
    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        Case::ALL.iter().map(|case| case.name()).collect()
    }

    /// Runs the case; false if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<bool> {
        let Some(case) = Case::named(name) else {
            return Ok(false);
        };
        let from = "ink@example.org".parse().map_err(|error: mail::AddressError| worker::Error::RustError(error.to_string()))?;
        case.run(&Provided(Cf::new(from, Fake::new()))).await;
        Ok(true)
    }
}
