//! What a request body says, as typed values.

use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use build::{Revision, Source};
use control::Handle;
use model::DeployId;
use model::git::Locator;
use serde::de::{DeserializeOwned, Visitor};
use serde::{Deserialize, Deserializer};

/// A value read from a JSON string by its `FromStr`, so a handle or a URL is checked where it
/// is read and a bad one is an error that says which.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed<T>(pub T);

struct Text<T>(PhantomData<T>);

impl<T> Visitor<'_> for Text<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    type Value = Parsed<T>;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("a string")
    }

    fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Parsed<T>, E> {
        text.parse().map(Parsed).map_err(E::custom)
    }
}

impl<'de, T> Deserialize<'de> for Parsed<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Text(PhantomData))
    }
}

/// Where a new site's content comes from.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Origin {
    /// A git repository: over HTTPS, or on the builder's disk when the operator allows it.
    Git {
        /// Where to fetch it.
        url: Parsed<Locator>,
        /// A branch, tag or commit; the remote's default when absent.
        revision: Option<Parsed<Revision>>,
    },
    /// Built folders sent with `PUT /v1/sites/{site}/deploy`.
    Upload,
}

impl From<Origin> for Source {
    fn from(origin: Origin) -> Self {
        match origin {
            Origin::Git { url, revision } => Self::Git { url: url.0, revision: revision.map(|revision| revision.0) },
            Origin::Upload => Self::Upload,
        }
    }
}

/// `POST /v1/sites`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddSite {
    /// The handle to claim.
    pub handle: Parsed<Handle>,
    /// Where the content comes from.
    pub source: Origin,
}

/// `POST /v1/sites/{site}/rollback`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rollback {
    /// The deploy to make live.
    pub deploy: DeployId,
}

/// `POST /v1/login`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ask {
    /// The address to send a sign-in link to.
    pub address: Parsed<mail::Address>,
}

/// `POST /v1/login/{token}`: the body may be empty. The address matters to the first-run link
/// only, which gives it to the owner's account so the owner can sign in by mail later.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Redeem {
    /// The address to give the account.
    pub address: Option<Parsed<mail::Address>>,
}

/// A body that is not the JSON a route reads.
#[derive(Debug)]
pub struct Unreadable {
    /// The JSON reader's own error, with the line and column.
    pub source: serde_json::Error,
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the request body is not the JSON this route reads")
    }
}

impl Unreadable {
    /// Reads `bytes` as the one `T` a route takes.
    pub fn read<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Self> {
        serde_json::from_slice(bytes).map_err(|source| Self { source })
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reads_a_git_site_with_its_url_checked() {
        let body = br#"{"handle":"diary","source":{"kind":"git","url":"https://codeberg.org/a/b","revision":"main"}}"#;
        let site: AddSite = Unreadable::read(body).unwrap();
        assert_eq!(site.handle.0.as_str(), "diary");
        assert!(matches!(Source::from(site.source), Source::Git { revision: Some(_), .. }));
        let bad = br#"{"handle":"diary","source":{"kind":"git","url":"ftp://x"}}"#;
        assert!(Unreadable::read::<AddSite>(bad).is_err());
        let extra = br#"{"handle":"diary","source":{"kind":"upload"},"admin":true}"#;
        assert!(Unreadable::read::<AddSite>(extra).is_err());
        let upload = br#"{"handle":"diary","source":{"kind":"upload"}}"#;
        assert_eq!(Source::from(Unreadable::read::<AddSite>(upload).unwrap().source), Source::Upload);
    }
}

/// `POST /v1/enrol`: a key asking to be enrolled. The key is the public half of a pair the
/// requester made; the private half never leaves its machine. The key type is the server's:
/// one today, `ed25519-v1`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enrolling {
    /// The public key, 43 characters of unpadded base64url.
    pub key: Parsed<sig::PublicKey>,
    /// What the person will see beside the fingerprint, such as `laptop`.
    pub label: Parsed<control::keys::Label>,
    /// `device` or `deploy`.
    pub purpose: Parsed<control::keys::Kind>,
    /// The site of a deploy key.
    #[serde(default)]
    pub site: Option<Parsed<control::Id<control::SiteKind>>>,
    /// The rights asked for, as `read,deploy`; the kind's defaults when absent.
    #[serde(default)]
    pub rights: Option<Parsed<control::keys::Rights>>,
    /// How long the key lives, as `90d`; the kind's default when absent.
    #[serde(default)]
    pub lifetime: Option<Parsed<control::signin::Ttl>>,
}

/// `POST /v1/approve/{code}`: the person says what they compared.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approve {
    /// The fingerprint they read on both screens.
    pub fingerprint: Parsed<sig::Fingerprint>,
}

/// `POST /v1/abuse`: a stranger reports a site. Nobody signs it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reporting {
    /// The handle of the site reported.
    pub handle: Parsed<control::Handle>,
    /// What kind of abuse.
    pub kind: control::AbuseKind,
    /// What happened.
    pub detail: control::Detail,
    /// How to reach the reporter.
    pub contact: Option<control::Contact>,
}

/// `POST /v1/sites/{site}/suspend`: the operator takes a site down.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suspending {
    /// Why.
    pub kind: control::AbuseKind,
    /// The report that led to it.
    pub report: Option<control::Id<control::ReportKind>>,
}
