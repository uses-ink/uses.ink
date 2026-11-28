//! The JWT an App signs to speak as itself.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jiff::{SignedDuration, Timestamp};
use serde_json::json;

use authority_forge::Secret;

use crate::{AppId, Signer};

/// A JWT of the App, RS256, with the claims GitHub asks for.
#[derive(Clone, Copy, Debug)]
pub struct Jwt;

/// A JWT that could not be made.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unsigned<S> {
    /// The claims could not be written.
    Json(serde_json::Error),
    /// The signer failed.
    Sign(S),
}

impl Jwt {
    /// How far before now `iat` is set: GitHub asks for a minute, for clocks that disagree.
    pub const BACKDATE: SignedDuration = SignedDuration::from_secs(60);
    /// How long it lives: nine minutes, under GitHub's ten.
    pub const LIFE: SignedDuration = SignedDuration::from_mins(9);

    /// The JWT of `app` at `now`, signed by `signer`. It is a credential: it comes back as a
    /// [`Secret`], and nothing keeps it.
    pub fn mint<S: Signer>(signer: &S, app: AppId, now: Timestamp) -> Result<Secret, Unsigned<S::Error>> {
        let header = json!({ "alg": "RS256", "typ": "JWT" });
        let claims = json!({
            "iat": (now - Self::BACKDATE).as_second(),
            "exp": (now + Self::LIFE).as_second(),
            "iss": app.0,
        });
        let mut message = Vec::new();
        for part in [header, claims] {
            if !message.is_empty() {
                message.push(b'.');
            }
            message.extend(URL_SAFE_NO_PAD.encode(serde_json::to_vec(&part).map_err(Unsigned::Json)?).into_bytes());
        }
        let signature = signer.sign(&message).map_err(Unsigned::Sign)?;
        message.push(b'.');
        message.extend(URL_SAFE_NO_PAD.encode(signature).into_bytes());
        Ok(Secret::new(message))
    }
}
