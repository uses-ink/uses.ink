//! The nonce store's suite, through a Durable Object.

use edge_api::{Claimed, Claiming, NonceCase, Nonces};
use edge_worker::{Remote, Serving};
use sig::{KeyId, Nonce};
use worker::{Date, DurableObject, Env, Request, Response, Result, State, durable_object};

/// The object of one key: a ledger started when the object is built, with a 60 second window.
#[durable_object]
pub struct NonceObject {
    serving: Serving,
}

impl DurableObject for NonceObject {
    fn new(_state: State, _env: Env) -> Self {
        Self { serving: Serving::new(Date::now().as_millis() / 1000, NonceCase::WINDOW) }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body = req.bytes().await?;
        match self.serving.handle(&body) {
            Ok(answer) => Response::from_bytes(answer),
            Err(error) => Response::error(error.to_string(), 400),
        }
    }
}

/// The nonce cases.
pub(crate) struct Ledgers<'a> {
    pub(crate) env: &'a Env,
}

impl Ledgers<'_> {
    /// The case beside the suite: what was created before the object was built is refused.
    const BOOT: &'static str = "refuses_what_was_created_before_the_object_was_built";

    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = NonceCase::ALL.iter().map(|case| case.name()).collect();
        names.push(Self::BOOT);
        names
    }

    /// The error of a failed set-up, as the rig's.
    fn rig(error: impl std::error::Error) -> worker::Error {
        worker::Error::RustError(error.to_string())
    }

    /// Seconds since the epoch, from the platform's clock.
    fn seconds() -> u64 {
        Date::now().as_millis() / 1000
    }

    /// Runs the case; false if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<bool> {
        let remote = Remote::new(self.env.durable_object("NONCES")?);
        if name == Self::BOOT {
            // an object for a key no case uses: the first claim builds it, and a request created
            // two minutes ago is older than its quiet period
            let now = Self::seconds();
            let claiming = Claiming {
                key: KeyId::new([0xB0; 16]),
                nonce: Nonce::new([0xB0; 16]),
                until: now - 120 + NonceCase::WINDOW,
                now,
                most: 4,
            };
            match remote.claim(claiming).await.map_err(Self::rig)? {
                Claimed::Quiet { seconds } => assert!(seconds >= 1, "a quiet period of {seconds} seconds"),
                other => return Err(worker::Error::RustError(format!("a request from before the object was built got {other:?}"))),
            }
            let fresh = Claiming { until: now + NonceCase::WINDOW, ..claiming };
            // the quiet period is 60 seconds from the build, so a request made now is still too early
            let verdict = remote.claim(fresh).await.map_err(Self::rig)?;
            assert!(matches!(verdict, Claimed::Quiet { .. }), "{verdict:?}");
        } else if let Some(case) = NonceCase::named(name) {
            case.run(&remote, Self::seconds() + 120).await;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
