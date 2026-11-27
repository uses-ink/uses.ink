//! What the API says about keys, as the CLI reads it, and the shapes `--json` prints.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A list answer: `{items: [...]}`.
#[derive(Debug, Deserialize)]
pub(crate) struct Items<T> {
    pub(crate) items: Vec<T>,
}

/// `POST /v1/enrol`: where the ask stands and what to show the person.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Enrolled {
    pub(crate) id: String,
    pub(crate) user_code: String,
    pub(crate) fingerprint: String,
    pub(crate) expires: Timestamp,
}

/// `GET /v1/enrol/{id}`.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Polled {
    pub(crate) state: String,
    pub(crate) expires: Option<Timestamp>,
}

/// Rights in a scope.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Grant {
    pub(crate) scope: String,
    pub(crate) id: String,
    pub(crate) rights: Vec<String>,
}

/// `GET /v1/whoami`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Whoami {
    pub(crate) account: String,
    pub(crate) kind: String,
    pub(crate) fingerprint: Option<String>,
    pub(crate) label: String,
    pub(crate) grants: Vec<Grant>,
    pub(crate) expires: Option<Timestamp>,
}

/// A key's end by revocation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Revoked {
    pub(crate) at: Timestamp,
    pub(crate) why: String,
}

/// A row of `GET /v1/keys`, and what `DELETE /v1/keys/{fingerprint}` answers.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Row {
    pub(crate) fingerprint: String,
    pub(crate) kind: String,
    pub(crate) label: String,
    pub(crate) grant: Grant,
    pub(crate) created: Timestamp,
    pub(crate) expires: Timestamp,
    pub(crate) last_used: Option<String>,
    pub(crate) revoked: Option<Revoked>,
}

/// What `ink login` and `ink key add` print with `--json`.
#[derive(Debug, Serialize)]
pub(crate) struct Joined<'a> {
    pub(crate) schema: &'static str,
    pub(crate) fingerprint: &'a str,
    pub(crate) label: &'a str,
    pub(crate) kind: &'a str,
    pub(crate) expires: Option<Timestamp>,
    /// Where the private key is kept, when it is kept on this machine.
    pub(crate) kept: Option<String>,
    /// The private key, only for a key meant for another machine and only with `--json`.
    pub(crate) private_key: Option<&'a str>,
}

/// What `ink whoami` prints with `--json`.
#[derive(Debug, Serialize)]
pub(crate) struct Who<'a> {
    pub(crate) schema: &'static str,
    #[serde(flatten)]
    pub(crate) who: &'a Whoami,
}

/// What `ink key list` prints with `--json`.
#[derive(Debug, Serialize)]
pub(crate) struct Listed<'a> {
    pub(crate) schema: &'static str,
    pub(crate) keys: &'a [Row],
}

/// What `ink key revoke` and `ink logout` print with `--json`.
#[derive(Debug, Serialize)]
pub(crate) struct Ended<'a> {
    pub(crate) schema: &'static str,
    pub(crate) key: &'a Row,
}
