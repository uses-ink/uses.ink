//! What the dashboard fails with: typed, with a stable code, a cause chain and a fix (decision
//! 0053). A failure is a page too, so every screen has an error state.

mod error;
mod page;

pub use error::{Cause, Failure, FieldError, Part, Unrouted};
pub use page::{Code, Fix};
