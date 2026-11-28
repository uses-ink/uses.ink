//! The conformance suite of the mail port (decision 0134): each case is a type with a `check`
//! over any [`Harness`], which wraps a [`Mailer`] and says what it delivered. [`Case`] lists them,
//! with a name each, as async functions any executor can drive (a Worker's event loop included,
//! which `pollster` cannot), and [`suite!`] turns every case into a native test.
//!
//! A mailer has no way to be read back, so each adapter's test says how: the capture keeps typed
//! messages, the stdout mailer writes text, the Cloudflare one hands a request to a transport that
//! keeps it. They all report what they delivered as [`Delivered`], and the cases compare that.
//!
//! ```ignore
//! mail_conformance::suite!(|| (MyHarness::new(), ()));
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use std::time::Duration;

use mail::{Address, LinkBase, Mailer, Message, Template};

#[doc(hidden)]
pub use pollster;

/// One message as a mailer handed it on: the three things a reader sees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivered {
    /// The recipient's address.
    pub to: String,
    /// The subject line.
    pub subject: String,
    /// The body, as text.
    pub body: String,
}

/// What a harness answers when asked to make its mailer fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusing {
    /// From now on every send fails, until it is told to stop.
    Told,
    /// The mailer cannot fail (the capture keeps what it is given in memory), so the case that
    /// needs a failure has nothing to check.
    Impossible,
}

/// A mailer and a way to read what it delivered.
pub trait Harness {
    /// The mailer under test.
    type Mailer: Mailer;

    /// The mailer.
    fn mailer(&self) -> &Self::Mailer;

    /// Everything delivered so far, in order.
    fn delivered(&self) -> Vec<Delivered>;

    /// Makes every send fail (or stops doing so), if the mailer can.
    fn refuse(&self, refusing: bool) -> Refusing;
}

/// Values the cases share.
struct Samples;

impl Samples {
    /// The token of every link the cases send: a secret that must reach the reader and no log.
    const TOKEN: &'static str = "tok-5f2c9a";

    /// A message of the sign-in template to `to`.
    fn login(to: &str) -> Message {
        let base: LinkBase = "https://ink.example/login".parse().unwrap();
        Message {
            to: to.parse().unwrap(),
            template: Template::Login { link: base.link(Self::TOKEN), valid: Duration::from_mins(15) },
        }
    }

    /// A message of the proof template to `to`.
    fn prove(to: &str) -> Message {
        let base: LinkBase = "https://ink.example/prove".parse().unwrap();
        Message {
            to: to.parse().unwrap(),
            template: Template::Prove { link: base.link(Self::TOKEN), valid: Duration::from_mins(30) },
        }
    }
}

/// A message arrives with its link.
#[derive(Debug, Clone, Copy)]
pub struct Arrives;

impl Arrives {
    /// One sign-in message is one delivery to the recipient, in lower case, with the template's
    /// subject and a body that holds the whole link and says how long it works.
    pub async fn check<H: Harness>(harness: &H) {
        assert!(harness.delivered().is_empty());
        harness.mailer().send(&Samples::login("Me@Example.ORG")).await.unwrap();
        let delivered = harness.delivered();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].to, "me@example.org");
        assert_eq!(delivered[0].subject, "Your sign-in link");
        assert!(
            delivered[0].body.contains(&format!("https://ink.example/login/{}", Samples::TOKEN)),
            "{:?}",
            delivered[0]
        );
        assert!(delivered[0].body.contains("15 minutes"), "{:?}", delivered[0]);
    }
}

/// Each message is its own template, in the order sent.
#[derive(Debug, Clone, Copy)]
pub struct Orders;

impl Orders {
    /// A proof, a sign-in and a proof again arrive as three, in order, each with its own subject
    /// and its own link, and the proof says it does not sign the reader in.
    pub async fn check<H: Harness>(harness: &H) {
        for message in [Samples::prove("a@x.org"), Samples::login("b@x.org"), Samples::prove("c@x.org")] {
            harness.mailer().send(&message).await.unwrap();
        }
        let delivered = harness.delivered();
        let to: Vec<&str> = delivered.iter().map(|one| one.to.as_str()).collect();
        assert_eq!(to, ["a@x.org", "b@x.org", "c@x.org"]);
        let subjects: Vec<&str> = delivered.iter().map(|one| one.subject.as_str()).collect();
        assert_eq!(subjects, ["Confirm this address", "Your sign-in link", "Confirm this address"]);
        assert!(delivered[0].body.contains("https://ink.example/prove/"), "{:?}", delivered[0]);
        assert!(delivered[0].body.contains("does not sign you in"), "{:?}", delivered[0]);
        assert!(delivered[1].body.contains("https://ink.example/login/"), "{:?}", delivered[1]);
    }
}

/// An address that could carry a header is no address.
#[derive(Debug, Clone, Copy)]
pub struct Refuses;

impl Refuses {
    /// What reaches a mailer is an [`Address`], so one that would add a header, a recipient or a
    /// second line is refused where it is read, before any mailer sees it.
    #[allow(clippy::unused_async, reason = "every case is async so one runner drives them all")]
    pub async fn check<H: Harness>(harness: &H) {
        for bad in [
            "",
            "no-at-sign",
            "a@b@c.org",
            "a@x.org\r\nBcc: b@y.org",
            "a@x.org\nSubject: x",
            "a b@x.org",
            "<a@x.org>",
            "a@x.org, b@y.org",
            "a@",
            "@x.org",
            "a@x..org",
        ] {
            assert!(bad.parse::<Address>().is_err(), "{bad:?} was taken for an address");
        }
        assert!(harness.delivered().is_empty());
    }
}

/// A failure is typed, delivers nothing and keeps the link out of what it says.
#[derive(Debug, Clone, Copy)]
pub struct Fails;

impl Fails {
    /// While the mailer is made to fail, `send` is an error whose text and debug output do not hold
    /// the token of the link, and nothing is delivered; once it stops, the same message goes
    /// through. A mailer that cannot fail has nothing to check.
    pub async fn check<H: Harness>(harness: &H) {
        if harness.refuse(true) == Refusing::Impossible {
            return;
        }
        let refused = harness.mailer().send(&Samples::login("me@x.org")).await.unwrap_err();
        let said = format!("{refused} {refused:?}");
        assert!(!said.contains(Samples::TOKEN), "the failure shows the link's token: {said}");
        assert!(harness.delivered().is_empty(), "a failed send delivered");
        harness.refuse(false);
        harness.mailer().send(&Samples::login("me@x.org")).await.unwrap();
        assert_eq!(harness.delivered().len(), 1);
    }
}

/// Lists the cases once, for the macros that need them: each entry is the [`Case`] variant, the type
/// that holds the check, and the name of the test.
#[doc(hidden)]
#[macro_export]
macro_rules! cases {
    ($callback:path, $($lead:tt)*) => {
        $callback! {
            $($lead)*
            [
                (Arrived, Arrives, delivers_a_message_with_its_link),
                (Ordered, Orders, keeps_each_template_in_the_order_sent),
                (Refused, Refuses, refuses_an_address_that_could_carry_a_header),
                (Failed, Fails, fails_typed_and_delivers_nothing)
            ]
        }
    };
}

macro_rules! define {
    ([$(($variant:ident, $case:ident, $name:ident)),*]) => {
        /// One case of the suite.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum Case {
            $(
                #[doc = concat!("`", stringify!($case), "`.")]
                $variant,
            )*
        }

        impl Case {
            /// Every case, in the order the suite runs them.
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            /// Its name: the name of its native test.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($name),)*
                }
            }

            /// The case named `name`, if there is one.
            pub fn named(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|case| case.name() == name)
            }

            /// Runs it on `harness`, which must be fresh. It panics at the first surprise.
            pub async fn run<H: Harness>(self, harness: &H) {
                match self {
                    $(Self::$variant => $case::check(harness).await,)*
                }
            }
        }
    };
}

cases!(define,);

/// The native tests of [`suite!`], one per case.
#[doc(hidden)]
#[macro_export]
macro_rules! tests {
    ($make:expr, [$(($variant:ident, $case:ident, $name:ident)),*]) => {
        $(
            #[test]
            fn $name() {
                let (harness, _guard) = ($make)();
                $crate::pollster::block_on($crate::Case::$variant.run(&harness));
            }
        )*
    };
}

/// Runs every case of the mail port's suite against the harness `$make` builds: a closure that
/// returns a fresh harness and whatever must outlive it (or `()`) as a pair. Each case is a test of
/// its own. The same cases are [`Case::ALL`], for an executor that is not `pollster`.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::cases!($crate::tests, $make,);
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_case_has_its_own_name() {
        let mut names: Vec<&str> = Case::ALL.iter().map(|case| case.name()).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
        assert_eq!(count, 4);
        assert!(Case::ALL.iter().all(|case| Case::named(case.name()) == Some(*case)));
    }
}
