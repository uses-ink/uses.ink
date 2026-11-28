//! A caller's key for tests that speak raw HTTP to `ink serve`: made here, added to the data
//! folder's `operator-keys` the way `ink login --data` does, and used to sign requests.

#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    reason = "shared by several test crates; tests stop at the first surprise"
)]

use std::io::Write as _;
use std::path::Path;

use sig::{Audience, Created, Method, Nonce, Parts, Seed, Signer, Target};

/// A key the server's operator file holds.
pub struct Keyed {
    signer: Signer,
    audience: Audience,
}

impl Keyed {
    /// Makes a key for the server at `audience` and appends its line to `<data>/operator-keys`.
    pub fn operator(data: &Path, audience: &str) -> Self {
        let draw = |seed: u8| [seed; 16];
        let mut first = draw(7);
        let mut second = draw(9);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos().to_le_bytes();
        first.copy_from_slice(&now);
        second.iter_mut().zip(now.iter().rev()).for_each(|(into, from)| *into = *from ^ 0x5a);
        let signer = Signer::from_seed(&Seed::from_draws(first, second));
        let mut file = std::fs::OpenOptions::new().append(true).create(true).open(data.join("operator-keys")).unwrap();
        writeln!(file, "ed25519-v1 {} test", signer.public()).unwrap();
        Self { signer, audience: Audience::parse(audience).unwrap() }
    }

    /// The `Authorization` header value for a request, signed now with a fresh nonce.
    pub fn sign(&self, method: &str, target: &str, body: &[u8]) -> String {
        static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut nonce = [0x11; 16];
        nonce[..8].copy_from_slice(&count.to_le_bytes());
        nonce[8..].copy_from_slice(&std::process::id().to_le_bytes().repeat(2));
        let method = Method::parse(method).unwrap();
        let target = Target::parse(target).unwrap();
        let created = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let parts = Parts { audience: &self.audience, method: &method, target: &target, body };
        self.signer.sign(&parts, Created::new(created), Nonce::new(nonce)).to_string()
    }
}
