//! The adapter against the mail port's suite.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_mail_cf as mail_cf;

use mail::{Address, Mailer as _};
use mail_cf::{Cf, Fake, Request};
use mail_conformance::{Delivered, Harness, Refusing};

/// The adapter over the fake provider, read back from what the provider took.
struct Provided(Cf<Fake>);

impl Provided {
    fn new() -> Self {
        Self(Cf::new("ink@example.org".parse().unwrap(), Fake::new()))
    }
}

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

mail_conformance::suite!(|| (Provided::new(), ()));

#[test]
fn sends_from_its_own_address_and_keeps_the_body_out_of_debug() {
    let provided = Provided::new();
    let base: mail::LinkBase = "https://ink.example/login".parse().unwrap();
    let template = mail::Template::Login { link: base.link("tok-77"), valid: std::time::Duration::from_mins(5) };
    let to: Address = "me@x.org".parse().unwrap();
    mail_conformance::pollster::block_on(provided.0.send(&mail::Message { to, template })).unwrap();
    let taken: Vec<Request> = provided.0.transport().taken();
    assert_eq!(taken[0].from.as_str(), "ink@example.org");
    assert!(taken[0].text.contains("tok-77"));
    assert!(!format!("{:?}", taken[0]).contains("tok-77"));
}
