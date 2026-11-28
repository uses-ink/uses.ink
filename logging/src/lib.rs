//! Logging for debugging and tracing, behind one small crate (decision 0065, docs/tracing.md).
//!
//! Every crate instruments through the macros here, which are `tracing`'s and this crate's own
//! typed ones, and none configures a subscriber. The lean part (the field types, the span
//! macros, the names, `Secret`) is all a plugin's dependency graph sees. The `layers` feature
//! adds the filter, the human, JSON-lines and timing layers and [`Logging`]; the `capture`
//! feature adds [`Capture`] for tests.
//!
//! Logging is a channel of its own: stderr or a file, never stdout and never inside a `--json`
//! document. A word of caution about the name: the product's "trace" is the replayed ladder of a
//! decision (`ink explain --why`). This crate is about the process, not about decisions.

mod field;
pub mod name;
mod tree;

#[cfg(any(feature = "layers", feature = "capture"))]
mod record;

#[cfg(feature = "capture")]
mod capture;

#[cfg(feature = "layers")]
mod filter;
#[cfg(feature = "layers")]
mod human;
#[cfg(feature = "layers")]
mod layer;
#[cfg(feature = "layers")]
mod lines;
#[cfg(feature = "layers")]
mod logging;
#[cfg(feature = "layers")]
mod timing;

pub use field::{Count, Digest, Failure, Field, Kind, Located, Redacted, Secret, Short, Shown, Size, Status, Took};
pub use name::{event, span};
/// A span that was entered and stays entered until it drops.
pub use tracing::span::EnteredSpan as Entered;
/// The macros of `tracing` that write events, so a crate depends on this one alone. An event
/// is written `logging::info!(name: event::UNIT_PLANNED, bytes = Size(n).field())`.
pub use tracing::{self, Instrument, debug, error, info, trace, warn};
pub use tracing::{Level, Span};
pub use tree::{Io, Stage, Unit};

#[cfg(feature = "capture")]
pub use capture::{Capture, Seen, Shape};
#[cfg(any(feature = "layers", feature = "capture"))]
pub use record::{Chain, Fields, Value};

#[cfg(feature = "layers")]
pub use filter::{Area, Directive, Filter, FilterError};
#[cfg(feature = "layers")]
pub use human::Format;
#[cfg(feature = "layers")]
pub use logging::{Color, Error, Guard, Logging, Process, Spans, Unknown, Variables, Vars};
#[cfg(feature = "layers")]
pub use timing::{Row, Timing};

#[cfg(all(test, feature = "capture"))]
mod test;
