//! Spike: build a registration challenge and an authentication start with `webauthn-rs-core`.

use url::Url;
use webauthn_rs_core::WebauthnCore;
use webauthn_rs_core::proto::{COSEAlgorithm, UserVerificationPolicy};

/// Makes a registration challenge, so the verifier code is linked in.
pub fn challenge() -> usize {
    let origin = Url::parse("https://app.uses.ink").expect("literal url");
    let core = WebauthnCore::new_unsafe_experts_only("uses.ink", "app.uses.ink", vec![origin], std::time::Duration::from_secs(300), None, None);
    let builder = core
        .new_challenge_register_builder(b"0123456789abcdef", "ada", "Ada")
        .expect("builder")
        .user_verification_policy(UserVerificationPolicy::Required)
        .require_resident_key(true)
        .credential_algorithms(vec![COSEAlgorithm::ES256]);
    let (creation, _state) = core.generate_challenge_register(builder).expect("challenge");
    Vec::<u8>::from(creation.public_key.challenge).len()
}
