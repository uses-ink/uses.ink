//! Who a request is signed as: the key from `INK_KEY`, `INK_KEY_FILE` or the keystore, and the
//! server's authority as the audience.

use clock::{Clock as _, Draw as _, System};
use sig::{Audience, Created, KeyId, Nonce, Parts, Seed, Signer, Target};

use super::store::Keystore;
use crate::deploy::{Failure, Needed};
use crate::settings::Environment;

/// A key and the server it is for.
pub(crate) struct Identity {
    signer: Signer,
    audience: Audience,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "Identity({})", self.signer.id().fingerprint())
    }
}

impl Identity {
    /// `seed`, for the server at `authority`.
    pub(crate) fn new(seed: &Seed, authority: &str) -> Result<Self, Failure> {
        let audience = Audience::parse(authority)
            .map_err(|source| Failure::Audience { written: authority.to_owned(), source })?;
        Ok(Self { signer: Signer::from_seed(seed), audience })
    }

    /// The key to sign as: `INK_KEY` (the key text, for a CI secret), else the file `INK_KEY_FILE`
    /// names, else the one `ink login` kept for this server.
    pub(crate) fn find(env: &impl Environment, authority: &str) -> Result<Self, Failure> {
        let parse = |from: &str, text: &str| {
            text.trim().parse::<Seed>().map_err(|source| Failure::KeyText { from: from.to_owned(), source })
        };
        let seed = if let Some(text) = env.get("INK_KEY").filter(|text| !text.is_empty()) {
            parse("`INK_KEY`", &text)?
        } else if let Some(path) = env.get("INK_KEY_FILE").filter(|path| !path.is_empty()) {
            let text = std::fs::read_to_string(&path)
                .map_err(|source| Failure::Store { source: super::StoreError::Io { path: path.clone().into(), source } })?;
            parse(&format!("`{path}`"), &text)?
        } else {
            // no home to keep a key in is no key kept
            let store = Keystore::of(env).map_err(|_| Failure::Setting { needed: Needed::Key })?;
            store
                .load(authority)
                .map_err(|source| Failure::Store { source })?
                .ok_or(Failure::Setting { needed: Needed::Key })?
        };
        Self::new(&seed, authority)
    }

    /// The key's id.
    pub(crate) fn id(&self) -> KeyId {
        self.signer.id()
    }

    /// The `Authorization` value for `method` of `target` with `body`, signed now.
    pub(crate) fn sign(&self, method: &str, target: &str, body: &[u8]) -> Result<String, Failure> {
        let method = sig::Method::parse(method).map_err(|source| Failure::Method { source })?;
        let target = Target::parse(target).map_err(|source| Failure::Target { source })?;
        let nonce = System.bytes().map_err(|source| Failure::Draw { source: Box::new(source) })?;
        let created = u64::try_from(System.now().as_second()).unwrap_or(0);
        let parts = Parts { audience: &self.audience, method: &method, target: &target, body };
        Ok(self.signer.sign(&parts, Created::new(created), Nonce::new(nonce)).to_string())
    }
}
