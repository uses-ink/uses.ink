//! The WebAuthn adapter: [`control::passkeys::Passkeys`] over `webauthn-rs-core` (decision 0068,
//! `spikes/passkeys`).
//!
//! The library is an "experts only" API whose defaults are not the server's rules, so this adapter
//! is where the rules are set and checked again:
//!
//! - **Discoverable, user verifying, ES256, attestation `none`**: set on the registration, and
//!   the reply's user verification flag is checked by the library and again here.
//! - **The counter** is not left to the library, which refuses every rollback (even on a synced
//!   passkey) with no way to turn that off. The library is handed the credential with counter 0, so
//!   it never judges one, and [`Counter::judge`] decides.
//! - **Backup eligibility** may not change after registration, in either direction.
//! - **The state** of a ceremony is the library's own serde form, JSON, held by the caller. The
//!   challenge is the library's, from its OS-seeded `rand` (the one randomness the server does not
//!   draw through `clock::Draw`, documented in `docs/auth.md`).
//! - **Errors** are mapped to a [`Reason`] each; the library's own error is kept as the source.

use std::time::Duration;

use control::passkeys::{
    Algorithm, Asking, Asserted, Attestation, Authenticating, Claim, Counter, Created, Creation, CredentialId,
    Enrolled, Holder, Named, Party, Passkeys, Person, Reason, Refusal, Registering, Relying, Requirement, Selection,
    State, Stored, Transport, Verified,
};
use control::{AccountKind, Id};
use url::Url;
use webauthn_rs_core::WebauthnCore;
use webauthn_rs_core::error::WebauthnError;
use webauthn_rs_core::proto::{
    AttestationConveyancePreference, AttestationFormat, AuthenticationExtensionsClientOutputs, AuthenticationState, AuthenticatorAssertionResponseRaw,
    AuthenticatorAttestationResponseRaw, AuthenticatorTransport, COSEAlgorithm, COSEKey, Credential, ParsedAttestation,
    PublicKeyCredential, RegisterPublicKeyCredential, RegisteredExtensions, RegistrationExtensionsClientOutputs,
    RegistrationState, UserVerificationPolicy,
};

/// What the adapter itself fails with, apart from refusing a reply.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failed {
    /// A configured origin is not a URL the library takes.
    Origin {
        /// The origin.
        origin: String,
        /// Why.
        source: url::ParseError,
    },
    /// The library would not make a challenge.
    Library(WebauthnError),
    /// The ceremony's state could not be written.
    State(serde_json::Error),
}

impl std::fmt::Display for Failed {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Origin { origin, .. } => write!(out, "`{origin}` is not a URL the verifier takes"),
            Self::Library(_) => out.write_str("the verifier would not make a challenge"),
            Self::State(_) => out.write_str("the state of the ceremony could not be written"),
        }
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Origin { source, .. } => Some(source),
            Self::Library(source) => Some(source),
            Self::State(source) => Some(source),
        }
    }
}

/// `webauthn-rs-core` as the server's [`Passkeys`].
pub struct Webauthn {
    core: WebauthnCore,
    relying: Relying,
}

impl std::fmt::Debug for Webauthn {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Webauthn").field("relying", &self.relying).finish_non_exhaustive()
    }
}

impl Webauthn {
    /// How long a browser waits for the person, in milliseconds. The server's own limit on a
    /// ceremony is `Registry::LIFE`; this is only the hint the browser is given.
    const WAIT: u32 = 300_000;

    /// A verifier for `relying`.
    pub fn new(relying: Relying) -> Result<Self, Failed> {
        let origins = relying
            .origins()
            .iter()
            .map(|origin| Url::parse(origin).map_err(|source| Failed::Origin { origin: origin.clone(), source }))
            .collect::<Result<Vec<_>, _>>()?;
        let core = WebauthnCore::new_unsafe_experts_only(
            relying.name(),
            relying.id(),
            origins,
            Duration::from_millis(u64::from(Self::WAIT)),
            None,
            None,
        );
        Ok(Self { core, relying })
    }

    /// The relying party it verifies for.
    pub const fn relying(&self) -> &Relying {
        &self.relying
    }

    /// The reason a library error is a refusal for.
    fn reason(error: &WebauthnError) -> Reason {
        match error {
            WebauthnError::InvalidRPOrigin | WebauthnError::CredentialCrossOrigin => Reason::Origin,
            WebauthnError::InvalidRPIDHash => Reason::RelyingParty,
            WebauthnError::UserNotVerified | WebauthnError::UserNotPresent => Reason::UserVerification,
            WebauthnError::MismatchedChallenge
            | WebauthnError::ChallengeNotFound
            | WebauthnError::InvalidClientDataType => Reason::Challenge,
            WebauthnError::CredentialNotFound
            | WebauthnError::CredentialExcludedFromRequest
            | WebauthnError::CredentialAlreadyExists => Reason::Credential,
            WebauthnError::CredentialPossibleCompromise => Reason::Counter,
            WebauthnError::CredentialBackupEligibilityInconsistent => Reason::Backup,
            WebauthnError::SignatureInvalid | WebauthnError::AuthenticationFailure => Reason::Signature,
            WebauthnError::COSEKeyInvalidAlgorithm
            | WebauthnError::CredentialAlteredAlgFromRequest
            | WebauthnError::CredentialInsecureCryptography
            | WebauthnError::COSEKeyInvalidType => Reason::Algorithm,
            _ => Reason::Malformed,
        }
    }

    fn refusal(error: WebauthnError) -> Refusal {
        Refusal::because(Self::reason(&error), error)
    }

    fn transports(reported: &[Transport]) -> Vec<AuthenticatorTransport> {
        reported
            .iter()
            .map(|transport| match transport {
                Transport::Usb => AuthenticatorTransport::Usb,
                Transport::Nfc => AuthenticatorTransport::Nfc,
                Transport::Ble => AuthenticatorTransport::Ble,
                Transport::Internal => AuthenticatorTransport::Internal,
                Transport::Hybrid => AuthenticatorTransport::Hybrid,
                Transport::Unknown => AuthenticatorTransport::Unknown,
            })
            .collect()
    }

    fn named(transport: &AuthenticatorTransport) -> Transport {
        match transport {
            AuthenticatorTransport::Usb => Transport::Usb,
            AuthenticatorTransport::Nfc => Transport::Nfc,
            AuthenticatorTransport::Ble => Transport::Ble,
            AuthenticatorTransport::Internal => Transport::Internal,
            AuthenticatorTransport::Hybrid => Transport::Hybrid,
            _ => Transport::Unknown,
        }
    }

    /// What was kept of a ceremony, read back, or a challenge refusal: a state that does not
    /// parse is no ceremony of this version.
    fn recalled<T: serde::de::DeserializeOwned>(state: &State) -> Result<T, Refusal> {
        serde_json::from_slice(state.as_slice()).map_err(|source| Refusal::because(Reason::Challenge, source))
    }

    /// The stored credential as the library knows it. Its counter is 0: the library refuses every
    /// rollback, and the server's policy is not that.
    fn known(stored: &Stored) -> Result<Credential, Refusal> {
        let key: COSEKey = serde_json::from_slice(stored.key.as_slice())
            .map_err(|source| Refusal::because(Reason::Algorithm, source))?;
        Ok(Credential {
            cred_id: stored.credential.as_slice().to_vec(),
            cred: key,
            counter: 0,
            transports: None,
            user_verified: true,
            backup_eligible: stored.eligible,
            backup_state: stored.backed_up,
            registration_policy: UserVerificationPolicy::Required,
            extensions: RegisteredExtensions::none(),
            attestation: ParsedAttestation::default(),
            attestation_format: AttestationFormat::None,
        })
    }

    /// The browser's assertion as the library reads it.
    fn received(reply: &Asserted) -> PublicKeyCredential {
        PublicKeyCredential {
            id: reply.id.to_string(),
            raw_id: reply.id.as_slice().to_vec(),
            response: AuthenticatorAssertionResponseRaw {
                authenticator_data: reply.authenticator.as_slice().to_vec(),
                client_data_json: reply.client_data.as_slice().to_vec(),
                signature: reply.signature.as_slice().to_vec(),
                user_handle: reply.handle.as_ref().map(|handle| handle.as_slice().to_vec()),
            },
            extensions: AuthenticationExtensionsClientOutputs::default(),
            type_: "public-key".to_owned(),
        }
    }

    fn kept<T: serde::Serialize>(state: &T) -> Result<State, Failed> {
        serde_json::to_vec(state).map(State::new).map_err(Failed::State)
    }
}

impl Passkeys for Webauthn {
    type Error = Failed;

    fn start_registration(
        &self,
        who: &Person,
        existing: &[CredentialId],
    ) -> Result<(Creation, Registering), Failed> {
        let name = who.label.as_str();
        let builder = self
            .core
            .new_challenge_register_builder(who.account.as_bytes(), name, name)
            .map_err(Failed::Library)?
            .attestation(AttestationConveyancePreference::None)
            .user_verification_policy(UserVerificationPolicy::Required)
            .exclude_credentials(Some(existing.iter().map(|id| id.as_slice().to_vec()).collect()))
            .require_resident_key(true)
            .credential_algorithms(vec![COSEAlgorithm::ES256]);
        let (made, state) = self.core.generate_challenge_register(builder).map_err(Failed::Library)?;
        let creation = Creation {
            rp: Party { id: self.relying.id().to_owned(), name: self.relying.name().to_owned() },
            user: Holder {
                id: control::passkeys::Bytes::new(who.account.as_bytes().to_vec()),
                name: name.to_owned(),
                display_name: name.to_owned(),
            },
            challenge: control::passkeys::Bytes::new(made.public_key.challenge),
            pub_key_cred_params: vec![Algorithm::ES256],
            timeout: Self::WAIT,
            exclude_credentials: existing.iter().cloned().map(Named::from).collect(),
            authenticator_selection: Selection {
                resident_key: Requirement::Required,
                user_verification: Requirement::Required,
            },
            attestation: Attestation::None,
        };
        Ok((creation, Registering(Self::kept(&state)?)))
    }

    fn finish_registration(&self, state: &Registering, reply: &Created) -> Result<Enrolled, Refusal> {
        let state: RegistrationState = Self::recalled(&state.0)?;
        let received = RegisterPublicKeyCredential {
            id: reply.id.to_string(),
            raw_id: reply.id.as_slice().to_vec(),
            response: AuthenticatorAttestationResponseRaw {
                attestation_object: reply.attestation.as_slice().to_vec(),
                client_data_json: reply.client_data.as_slice().to_vec(),
                transports: Some(Self::transports(&reply.transports)),
            },
            type_: "public-key".to_owned(),
            extensions: RegistrationExtensionsClientOutputs::default(),
        };
        let made = self.core.register_credential(&received, &state, None).map_err(Self::refusal)?;
        if !made.user_verified {
            return Err(Reason::UserVerification.into());
        }
        let key = serde_json::to_vec(&made.cred).map_err(|source| Refusal::because(Reason::Malformed, source))?;
        Ok(Stored {
            credential: CredentialId::new(made.cred_id),
            key: control::passkeys::Bytes::new(key),
            counter: made.counter,
            eligible: made.backup_eligible,
            backed_up: made.backup_state,
            transports: made.transports.iter().flatten().map(Self::named).collect(),
        })
    }

    fn start_login(&self) -> Result<(Asking, Authenticating), Failed> {
        let builder = self
            .core
            .new_challenge_authenticate_builder(Vec::new(), Some(UserVerificationPolicy::Required))
            .map_err(Failed::Library)?;
        let (made, state) = self.core.generate_challenge_authenticate(builder).map_err(Failed::Library)?;
        let asking = Asking {
            challenge: control::passkeys::Bytes::new(made.public_key.challenge),
            rp_id: self.relying.id().to_owned(),
            timeout: Self::WAIT,
            user_verification: Requirement::Required,
            allow_credentials: Vec::new(),
        };
        Ok((asking, Authenticating(Self::kept(&state)?)))
    }

    fn who(&self, reply: &Asserted) -> Result<Claim, Refusal> {
        let handle = reply.handle.as_ref().ok_or(Reason::Credential)?;
        let bytes = <[u8; 16]>::try_from(handle.as_slice()).map_err(|_| Refusal::new(Reason::Credential))?;
        let account: Id<AccountKind> = Id::from_bytes(bytes);
        Ok(Claim { credential: reply.id.clone(), account })
    }

    fn finish_login(
        &self,
        state: &Authenticating,
        reply: &Asserted,
        stored: &Stored,
    ) -> Result<Verified, Refusal> {
        let state: AuthenticationState = Self::recalled(&state.0)?;
        let known = Self::known(stored)?;
        let received = Self::received(reply);
        let result = self.core.authenticate_single_credential(&received, &state, &known).map_err(Self::refusal)?;
        if !result.user_verified() {
            return Err(Reason::UserVerification.into());
        }
        if result.backup_eligible() != stored.eligible {
            return Err(Reason::Backup.into());
        }
        let judged = Counter::judge(stored.counter, result.counter(), stored.eligible);
        if judged == Counter::Refused {
            return Err(Reason::Counter.into());
        }
        Ok(Verified {
            credential: stored.credential.clone(),
            counter: result.counter(),
            backed_up: result.backup_state(),
            judged,
        })
    }
}
