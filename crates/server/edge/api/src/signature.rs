//! Checking `Ink-Sig-1` signed requests (decision 0068, `docs/auth.md` section 5).
//!
//! [`Signatures`] is the [`Credentials`] adapter. In order, and each refusal the one the design's
//! table gives: the **skew** window, the **boot barrier**, the **key** (an operator's line, an
//! account's key, or a key waiting to be enrolled), the **strict signature** over the signed
//! text with the server's own audience, the key being **live**, and only then the **nonce**
//! claim, so a caller who cannot sign cannot fill the store. The body's digest is checked by the
//! API once the body is read.
//!
//! Time is the [`clock::Clock`] port and nothing here reads `std::time`. The nonce store is
//! memory; a restart loses it, and the **boot barrier** closes that: for `skew` seconds after the
//! server starts, any request made before `boot + skew` is answered `restarting`, so a request
//! captured before the restart cannot be replayed into the empty store.

use std::convert::Infallible;
use std::error::Error;
use std::fmt;

use clock::Clock;
use control::keys::{Audit, Effective, Grants, Keys, Kind, State};
use control::{AccountKind, Id};
use db::Db;
use jiff::Timestamp;
use sig::{Audience, Claim, Covered, KeyType, Verifying};

use crate::auth::credential::{Credentials, Offered, Presented, Principal, Verdict, Via};
use crate::fault::Denied;
use crate::nonce::{Claimed, Claiming, Nonces, Remembered};
use crate::operator::{OperatorKey, Operators};

/// The skew window: how many seconds either side of the server's time a request's `created` may
/// be, and the length of the quiet after a restart. The operator's to set, with a ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window(u64);

/// A window that is not allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowError {
    /// No window at all.
    Zero,
    /// Over the ceiling.
    Over {
        /// What was asked.
        asked: u64,
    },
}

impl fmt::Display for WindowError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => out.write_str("the skew window is at least one second"),
            Self::Over { asked } => write!(out, "the skew window is at most {} seconds, not {asked}", Window::CEILING),
        }
    }
}

impl Error for WindowError {}

impl Window {
    /// The default: 60 seconds.
    pub const DEFAULT: Self = Self(60);

    /// The most a window may be: 300 seconds.
    pub const CEILING: u64 = 300;

    /// A window of `seconds`.
    pub const fn new(seconds: u64) -> Result<Self, WindowError> {
        match seconds {
            0 => Err(WindowError::Zero),
            1..=Self::CEILING => Ok(Self(seconds)),
            asked => Err(WindowError::Over { asked }),
        }
    }

    /// The seconds.
    pub const fn seconds(self) -> u64 {
        self.0
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What the operator decides about verifying signatures.
#[derive(Clone, Debug)]
pub struct Config {
    /// The authority this server answers to: what a signature is made for. Not the `Host` header.
    pub audience: Audience,
    /// The skew window, and the quiet after a restart.
    pub window: Window,
    /// How many nonces are remembered for one key at once; over it, the key is refused until some
    /// age out. Default 1024.
    pub nonces: usize,
    /// The key types this server accepts. Retiring one is a change here.
    pub types: Vec<KeyType>,
    /// The account an operator key signs in as: the owner of a self-host.
    pub operator: Id<AccountKind>,
}

impl Config {
    /// The nonces remembered per key when none is set.
    pub const NONCES: usize = 1024;

    /// A configuration for `audience` and the `operator` account, with the defaults.
    pub fn new(audience: Audience, operator: Id<AccountKind>) -> Self {
        Self { audience, window: Window::DEFAULT, nonces: Self::NONCES, types: vec![KeyType::Ed25519V1], operator }
    }
}

/// Why a signature could not be checked at all (as opposed to being refused).
#[derive(Debug)]
#[non_exhaustive]
pub enum Failed<D, O, N = Infallible> {
    /// The key rows could not be read.
    Keys(control::keys::Error<D>),
    /// The operator's keys could not be read.
    Operators(O),
    /// The nonce store could not be reached.
    Nonces(N),
}

impl<D, O, N> fmt::Display for Failed<D, O, N> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keys(_) => out.write_str("the keys could not be read"),
            Self::Operators(_) => out.write_str("the operator's keys could not be read"),
            Self::Nonces(_) => out.write_str("the nonce store could not be reached"),
        }
    }
}

impl<D: Error + 'static, O: Error + 'static, N: Error + 'static> Error for Failed<D, O, N> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Keys(source) => Some(source),
            Self::Operators(source) => Some(source),
            Self::Nonces(source) => Some(source),
        }
    }
}

/// Where a key was found.
enum Found {
    Operator(OperatorKey),
    Key(Box<control::keys::Key>),
    Pending(control::keys::Enrolment),
}

/// The [`Credentials`] adapter for signed requests: the operator's file, the account's keys and
/// keys waiting to be enrolled, over a clock.
#[derive(Debug)]
pub struct Signatures<D, K, O, N = Remembered> {
    keys: Keys<D>,
    clock: K,
    operators: O,
    nonces: N,
    config: Config,
    boot: Timestamp,
}

impl<D: Db, K: Clock, O: Operators> Signatures<D, K, O> {
    /// A verifier over `keys` and the `operators`, started now: its boot barrier begins here. Its
    /// nonces are in memory.
    pub fn new(keys: Keys<D>, parts: (K, O), config: Config) -> Self {
        Self::with_nonces(keys, parts, config, Remembered::new())
    }
}

impl<D: Db, K: Clock, O: Operators, N: Nonces> Signatures<D, K, O, N> {
    /// A verifier over `keys` and the `operators` that remembers nonces in `nonces`, started now.
    pub fn with_nonces(keys: Keys<D>, parts: (K, O), config: Config, nonces: N) -> Self {
        let (clock, operators) = parts;
        let boot = clock.now();
        Self { keys, clock, operators, nonces, config, boot }
    }

    /// The same verifier with its boot barrier at `boot` instead of now. A nonce store that
    /// outlives the process (a Durable Object) keeps the barrier itself, from when it was built, so
    /// a process that restarts often does not close the door each time.
    #[must_use]
    pub const fn booted(mut self, boot: Timestamp) -> Self {
        self.boot = boot;
        self
    }

    /// The configuration.
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// The time the barrier lifts: requests made before it are answered `restarting`.
    pub fn quiet_until(&self) -> u64 {
        Self::seconds(self.boot) + self.config.window.seconds()
    }

    fn seconds(time: Timestamp) -> u64 {
        u64::try_from(time.as_second()).unwrap_or(0)
    }

    async fn find(&self, id: sig::KeyId) -> Result<Option<Found>, Failed<D::Error, O::Error, N::Error>> {
        let operators = self.operators.keys().map_err(Failed::Operators)?;
        if let Some(found) = operators.into_iter().find(|key| key.public.id(key.key_type) == id) {
            return Ok(Some(Found::Operator(found)));
        }
        if let Some(key) = self.keys.key(id).await.map_err(Failed::Keys)? {
            return Ok(Some(Found::Key(Box::new(key))));
        }
        Ok(self.keys.waiting_key(id).await.map_err(Failed::Keys)?.map(Found::Pending))
    }

    /// The principal of a key that passed every check.
    fn principal(&self, found: Found) -> Verdict {
        match found {
            Found::Operator(key) => {
                let account = self.config.operator;
                let via = Via::Operator { label: key.label.to_string() };
                Verdict::Granted(Principal {
                    account,
                    person: account,
                    via,
                    effective: Effective::own(Grants::owner(account)),
                    expires: None,
                })
            }
            Found::Key(key) => {
                let effective = match key.kind {
                    // a device key acts for a person: what both allow
                    Kind::Device => Effective::within(key.grants(), Grants::owner(key.account)),
                    // a deploy key is a service identity: its own grants stand
                    _ => Effective::own(key.grants()),
                };
                let via = Via::Key { id: key.id, kind: key.kind, label: key.label.to_string() };
                Verdict::Granted(Principal {
                    account: key.account,
                    person: key.account,
                    via,
                    effective,
                    expires: Some(key.expires),
                })
            }
            Found::Pending(enrolment) => Verdict::Enrolling(enrolment.key()),
        }
    }
}

impl<D: Db, K: Clock, O: Operators, N: Nonces> Signatures<D, K, O, N> {
    /// Why a key that proved itself may not be used: it was revoked or has ended. Only a caller
    /// who signed may hear it, so the answer tells a key's holder and nobody else.
    async fn ended(
        &self,
        found: &Found,
        now: Timestamp,
    ) -> Result<Option<Denied>, Failed<D::Error, O::Error, N::Error>> {
        let Found::Key(key) = found else { return Ok(None) };
        Ok(match key.state(now) {
            State::Live => None,
            State::Revoked { at, .. } => Some(Denied::Revoked { since: at }),
            _ => {
                self.keys.record(key.account, Audit::KeyExpired { key: key.id }, now).await.map_err(Failed::Keys)?;
                Some(Denied::Expired { since: key.expires })
            }
        })
    }

    /// Claims the nonce: why not when it was seen or the key has too many.
    async fn claimed(
        &self,
        found: &Found,
        claim: &Claim,
        now: Timestamp,
    ) -> Result<Option<Denied>, Failed<D::Error, O::Error, N::Error>> {
        let (secs, window) = (Self::seconds(now), self.config.window.seconds());
        let until = claim.created.seconds() + window;
        let claiming = Claiming { key: claim.key, nonce: claim.nonce, until, now: secs, most: self.config.nonces };
        Ok(match self.nonces.claim(claiming).await.map_err(Failed::Nonces)? {
            Claimed::Fresh => None,
            Claimed::Replay => {
                if let Found::Key(key) = found {
                    self.keys
                        .record(key.account, Audit::ReplayDetected { key: key.id }, now)
                        .await
                        .map_err(Failed::Keys)?;
                }
                Some(Denied::Replayed)
            }
            Claimed::Quiet { seconds } => Some(Denied::Restarting { seconds }),
            _ => Some(Denied::Flooded { window }),
        })
    }
}

impl<D: Db, K: Clock, O: Operators, N: Nonces> Credentials for Signatures<D, K, O, N> {
    type Error = Failed<D::Error, O::Error, N::Error>;

    async fn verify(&self, offered: &Offered, at: &Presented<'_>) -> Result<Verdict, Self::Error> {
        // the passkey is reserved: refused as a signature that does not verify is
        let Offered::Signed(signed) = offered else { return Ok(Verdict::Refused(Denied::Refused)) };
        let (now, window) = (self.clock.now(), self.config.window.seconds());
        let (secs, claim) = (Self::seconds(now), &signed.claim);
        if let Err(skew) = claim.created.within(secs, window) {
            return Ok(Verdict::Refused(Denied::Skewed { skew: skew.distance, window, server_time: now }));
        }
        let quiet = self.quiet_until();
        if claim.created.seconds() < quiet {
            return Ok(Verdict::Refused(Denied::Restarting { seconds: quiet.saturating_sub(secs).max(1) }));
        }
        let Some(found) = self.find(claim.key).await? else { return Ok(Verdict::Refused(Denied::Refused)) };
        let (public, key_type) = match &found {
            Found::Operator(key) => (key.public, key.key_type),
            Found::Key(key) => (key.public, key.key_type),
            Found::Pending(enrolment) => (enrolment.public, enrolment.key_type),
        };
        let verified =
            Verifying::new(&public).ok().filter(|_| self.config.types.contains(&key_type)).is_some_and(|key| {
                let covered = Covered { audience: &self.config.audience, method: at.method, target: at.target, claim };
                key.verify(&covered, &signed.signature).is_ok()
            });
        if !verified {
            return Ok(Verdict::Refused(Denied::Refused));
        }
        // the caller proved the key: only now may it hear that the key ended, or touch the store
        if let Some(denied) = self.ended(&found, now).await? {
            return Ok(Verdict::Refused(denied));
        }
        if let Some(denied) = self.claimed(&found, claim, now).await? {
            return Ok(Verdict::Refused(denied));
        }
        if let Found::Key(key) = &found {
            self.keys.used(key.id, Keys::<D>::day(now)).await.map_err(Failed::Keys)?;
        }
        Ok(self.principal(found))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_window_has_a_floor_and_a_ceiling() {
        assert_eq!(Window::new(0), Err(WindowError::Zero));
        assert_eq!(Window::new(301), Err(WindowError::Over { asked: 301 }));
        assert_eq!(Window::new(300).map(Window::seconds), Ok(300));
        assert_eq!(Window::default().seconds(), 60);
    }
}
