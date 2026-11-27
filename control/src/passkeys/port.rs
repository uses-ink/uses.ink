//! The port: what the server needs of a WebAuthn verifier, and nothing about any library's types.

use std::error::Error;

use super::{
    Asking, Asserted, Authenticating, Claim, Created, CredentialId, Creation, Enrolled, Person, Refusal,
    Registering, Stored, Verified,
};

/// A WebAuthn verifier (decision 0068, `docs/auth.md` section 4).
///
/// The server's rules are in the contract, not in the adapter's library: discoverable
/// credentials, user verification required at registration and at every sign-in, ES256 only,
/// attestation `none`, the origin and the relying party id checked, the signature counter judged
/// by [`super::Counter::judge`], and backup eligibility unchanged since registration. A
/// ceremony's state is kept by the caller, single use and expiring; the verifier keeps none.
///
/// Calls are synchronous: no I/O happens in verification.
pub trait Passkeys {
    /// What the verifier itself fails with: a source of randomness, a state that cannot be written.
    type Error: Error + 'static;

    /// Starts a registration of a passkey for `who`, naming the credentials the account has so
    /// the authenticator does not make a second one.
    fn start_registration(
        &self,
        who: &Person,
        existing: &[CredentialId],
    ) -> Result<(Creation, Registering), Self::Error>;

    /// Verifies the browser's answer to a registration.
    fn finish_registration(&self, state: &Registering, reply: &Created) -> Result<Enrolled, Refusal>;

    /// Starts a sign-in that asks for no username.
    fn start_login(&self) -> Result<(Asking, Authenticating), Self::Error>;

    /// Which credential and account an assertion says it is for. Nothing is verified: the caller
    /// loads the stored credential by this id, then calls [`Passkeys::finish_login`].
    fn who(&self, reply: &Asserted) -> Result<Claim, Refusal>;

    /// Verifies an assertion against the stored credential.
    fn finish_login(
        &self,
        state: &Authenticating,
        reply: &Asserted,
        stored: &Stored,
    ) -> Result<Verified, Refusal>;
}
