//! The control plane's core: accounts, sites and handles, deploys with their rollback pointers,
//! build jobs, credentials and the site's event log, over the five ports (decision 0045).
//!
//! - The domain is typed values ([`Account`], [`Site`], [`Deploy`], [`Job`], [`Credential`],
//!   [`Event`]) read back from rows by `TryFrom<&Row>`, with typed errors that keep their
//!   sources ([`Error`], [`Refusal`], [`Unreadable`]).
//! - The use cases are methods of [`Control`], generic over the ports ([`Ports`]) and knowing no
//!   adapter: add an account or a site, deploy a build's result, list deploys, roll back,
//!   resume, export, request and work builds, store credentials, remove a site with a
//!   [`Receipt`].
//! - The rows are made by the [`Domain`] migration, built from typed definitions (no SQL text).
//!
//! The caller draws every id and says what time it is, so no use case reads a clock or random
//! numbers, and the same calls make the same rows in any test. Only atomic batches and
//! single-statement compare-and-set are used (decision 0030): there is no interactive
//! transaction, so what must hold is in a statement's `WHERE`.

mod account;
mod case;
mod control;
mod credential;
mod deploy;
mod error;
mod event;
mod fields;
mod handle;
mod id;
mod job;
pub mod keys;
mod listed;
mod migration;
mod origin;
pub mod passkeys;
mod ports;
mod receipt;
pub mod signin;
mod site;
#[allow(missing_docs, reason = "each variant is the table or column of its name")]
pub mod table;

pub use account::{Account, New as NewAccount};
pub use case::export::{Exported, Sink, Summary};
pub use case::serve::Serving;
pub use control::Control;
pub use credential::{Credential, New as NewCredential, Use as Purpose};
pub use deploy::{Deploy, Live, New as NewDeploy, Published, Recorded, Resumed};
pub use error::{Error, Failed, Refusal, Unreadable};
pub use event::{Event, Unwritten};
pub use handle::{Handle, HandleError};
pub use id::{
    Account as AccountKind, Credential as CredentialKind, Enrolment as EnrolmentKind, Factor as FactorKind, Id,
    IdError, Kind, Session as SessionKind, Site as SiteKind,
};
pub use job::{Dropped, Job, New as NewJob, Requested, State as JobState, Worked};
pub use listed::{Canonical, Listed};
pub use migration::{Domain, Remembered};
pub use origin::{Origin, OriginError};
pub use ports::Ports;
pub use receipt::Receipt;
pub use signin::SignIn;
pub use site::{New as NewSite, Site, State as SiteState};
