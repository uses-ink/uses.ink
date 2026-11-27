//! The dashboard: five screens (Site, What we understood, Deploys, Settings, Account) as typed
//! view models and pure rendering, and the trait the edge mounts them through (decision 0053).
//!
//! - A page is a function of a typed view model to `unspliced` values: no string is spliced, and the
//!   one script (`passkey.js`, for the passkey buttons) is static text. Interaction is full-page forms answered by a redirect, `<details>` for
//!   open and close, and one streamed page for a running build's log.
//! - The crate knows no HTTP server and no adapter. The edge reads a request into a [`Request`],
//!   calls [`Dashboard::respond`] over its implementation of [`Screens`], and sends the [`Reply`].
//! - Every screen has an empty state that says what to do, and every failure is a [`Failure`]
//!   with a code, a cause chain and a fix, shown as a page.

mod dashboard;
mod failure;
mod form;
mod mark;
mod route;
mod said;
mod shell;
mod status;
mod stream;
mod style;
pub mod view;

pub use dashboard::{Body, Dashboard, Download, Logged, Reply, Screens};
pub use failure::{Cause, Code, Failure, FieldError, Fix, Part, Unrouted};
pub use form::{Action, Asked, Field, Form, Method, Request};
pub use mark::{Ink, Rule};
pub use route::{Only, Path, Route, Screen};
pub use shell::{Document, Frame, View};
pub use status::Status;
pub use stream::{Follow, Streamed};
