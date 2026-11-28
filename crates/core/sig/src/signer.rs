//! Making a signature and checking one, strictly.

use std::fmt;

use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};

use crate::covered::{Audience, Covered, Method};
use crate::key::{KeyError, KeyId, KeyType, PublicKey, Seed};
use crate::signed::{Claim, Created, Digest, Nonce, Signature, Signed};
use crate::target::Target;

/// A signature that does not verify.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forged;

impl fmt::Display for Forged {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the signature does not verify")
    }
}

impl std::error::Error for Forged {}

/// What a request is, for the signer: the audience, the method, the canonical target and the
/// body.
#[derive(Clone, Copy, Debug)]
pub struct Parts<'a> {
    /// The authority of the server.
    pub audience: &'a Audience,
    /// The method.
    pub method: &'a Method,
    /// The canonical target.
    pub target: &'a Target,
    /// The body, empty when there is none.
    pub body: &'a [u8],
}

/// The private key, ready to sign. It is made from a 32-byte seed only, never from a separately
/// supplied public key (RUSTSEC-2022-0093, the double public key oracle).
pub struct Signer {
    key: SigningKey,
}

impl fmt::Debug for Signer {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Signer({})", self.id().fingerprint())
    }
}

impl Signer {
    /// The key a seed makes.
    pub fn from_seed(seed: &Seed) -> Self {
        Self { key: SigningKey::from_bytes(seed.bytes()) }
    }

    /// The public half.
    pub fn public(&self) -> PublicKey {
        PublicKey::new(self.key.verifying_key().to_bytes())
    }

    /// The id of the key.
    pub fn id(&self) -> KeyId {
        self.public().id(KeyType::Ed25519V1)
    }

    /// The header for `parts`, made at `created` with `nonce`.
    pub fn sign(&self, parts: &Parts<'_>, created: Created, nonce: Nonce) -> Signed {
        let claim = Claim { key: self.id(), created, nonce, digest: Digest::of(parts.body) };
        let covered = Covered { audience: parts.audience, method: parts.method, target: parts.target, claim: &claim };
        let signature = Signature::new(self.key.sign(&covered.bytes()).to_bytes());
        Signed { claim, signature }
    }
}

/// A public key ready to verify, strictly: a non-canonical `S` (malleability), a small-order or
/// non-canonical public key and a cofactored equation are all refused.
#[derive(Clone, Debug)]
pub struct Verifying {
    key: VerifyingKey,
}

impl Verifying {
    /// The verifier of `key`, or why it is none.
    pub fn new(key: &PublicKey) -> Result<Self, KeyError> {
        VerifyingKey::from_bytes(key.bytes()).map(|key| Self { key }).map_err(|_| KeyError::Point)
    }

    /// Whether `signature` is this key's over `covered`.
    pub fn verify(&self, covered: &Covered<'_>, signature: &Signature) -> Result<(), Forged> {
        let signature = ed25519_dalek::Signature::from_bytes(signature.bytes());
        self.key.verify_strict(&covered.bytes(), &signature).map_err(|_| Forged)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn hex<const N: usize>(text: &str) -> [u8; N] {
        let bytes: Vec<u8> = (0..text.len() / 2)
            .map(|at| u8::from_str_radix(text.get(at * 2..at * 2 + 2).unwrap_or("zz"), 16).unwrap())
            .collect();
        <[u8; N]>::try_from(bytes).unwrap()
    }

    #[test]
    fn rfc_8032_test_1_gives_the_public_key_and_the_signature() {
        let seed = Seed::new(hex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"));
        let signer = Signer::from_seed(&seed);
        assert_eq!(
            signer.public().bytes(),
            &hex::<32>("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
        );
        let signature = signer.key.sign(b"");
        let expected = hex::<64>(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        );
        assert_eq!(signature.to_bytes(), expected);
    }

    fn parts<'a>(audience: &'a Audience, method: &'a Method, target: &'a Target, body: &'a [u8]) -> Parts<'a> {
        Parts { audience, method, target, body }
    }

    #[test]
    fn a_signature_verifies_and_every_covered_component_matters() {
        let signer = Signer::from_seed(&Seed::new([5; 32]));
        let verifying = Verifying::new(&signer.public()).unwrap();
        let audience = Audience::parse("api.uses.ink").unwrap();
        let method = Method::parse("POST").unwrap();
        let target = Target::parse("/v1/sites?a=1").unwrap();
        let issued =
            signer.sign(&parts(&audience, &method, &target, b"{}"), Created::new(1_795_000_000), Nonce::new([9; 16]));
        let verdict = |aud: &Audience, verb: &Method, path: &Target, claim: &Claim| {
            verifying.verify(&Covered { audience: aud, method: verb, target: path, claim }, &issued.signature)
        };
        assert_eq!(verdict(&audience, &method, &target, &issued.claim), Ok(()));
        assert_eq!(verdict(&Audience::parse("api.other.ink").unwrap(), &method, &target, &issued.claim), Err(Forged));
        assert_eq!(verdict(&audience, &Method::parse("PUT").unwrap(), &target, &issued.claim), Err(Forged));
        assert_eq!(verdict(&audience, &method, &Target::parse("/v1/sites?a=2").unwrap(), &issued.claim), Err(Forged));
        let mut moved = issued.claim;
        moved.digest = Digest::of(b"{ }");
        assert_eq!(verdict(&audience, &method, &target, &moved), Err(Forged));
        moved = issued.claim;
        moved.created = Created::new(1_795_000_001);
        assert_eq!(verdict(&audience, &method, &target, &moved), Err(Forged));
        moved = issued.claim;
        moved.nonce = Nonce::new([8; 16]);
        assert_eq!(verdict(&audience, &method, &target, &moved), Err(Forged));
        moved = issued.claim;
        moved.key = KeyId::new([1; 16]);
        assert_eq!(verdict(&audience, &method, &target, &moved), Err(Forged));
        let mut bad = *issued.signature.bytes();
        bad[0] ^= 1;
        let covered = Covered { audience: &audience, method: &method, target: &target, claim: &issued.claim };
        assert_eq!(verifying.verify(&covered, &Signature::new(bad)), Err(Forged));
    }

    #[test]
    fn a_small_order_key_and_a_non_canonical_s_are_refused() {
        // the identity point: a small-order public key verifies nothing under the strict check
        let identity = PublicKey::new({
            let mut bytes = [0_u8; 32];
            bytes[0] = 1;
            bytes
        });
        let verifying = Verifying::new(&identity).unwrap();
        let audience = Audience::parse("a").unwrap();
        let (method, target) = (Method::parse("GET").unwrap(), Target::parse("/").unwrap());
        let claim = Claim {
            key: KeyId::new([0; 16]),
            created: Created::new(1),
            nonce: Nonce::new([0; 16]),
            digest: Digest::of(b""),
        };
        let covered = Covered { audience: &audience, method: &method, target: &target, claim: &claim };
        let mut signature = [0_u8; 64];
        signature[0] = 1;
        assert_eq!(verifying.verify(&covered, &Signature::new(signature)), Err(Forged));
        // S = L (the group order) is not canonical and is refused whatever the key
        let signer = Signer::from_seed(&Seed::new([5; 32]));
        let honest = signer.sign(&parts(&audience, &method, &target, b""), Created::new(1), Nonce::new([0; 16]));
        let mut malleable = *honest.signature.bytes();
        let order: [u8; 32] = hex("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
        let mut carry = 0_u16;
        for (at, byte) in malleable.iter_mut().skip(32).enumerate() {
            let sum = u16::from(*byte) + u16::from(order[at]) + carry;
            *byte = u8::try_from(sum & 255).unwrap_or_default();
            carry = sum >> 8;
        }
        let covered = Covered { audience: &audience, method: &method, target: &target, claim: &honest.claim };
        assert_eq!(Verifying::new(&signer.public()).unwrap().verify(&covered, &Signature::new(malleable)), Err(Forged));
    }
}
