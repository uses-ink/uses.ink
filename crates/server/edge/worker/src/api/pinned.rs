//! The operator's keys as the Worker is given them: the text of an authorized-keys file in a
//! variable, read once when the Worker starts.

use std::convert::Infallible;

use edge_api::{OperatorError, OperatorKey, Operators, Roll};
use logging::Field as _;

/// The keys of the operator, fixed for the life of the isolate. A line that does not read is
/// kept and said, never dropped silently.
#[derive(Debug, Default)]
pub struct Pinned {
    keys: Vec<OperatorKey>,
    skipped: Vec<OperatorError>,
}

impl Pinned {
    /// The keys written in `text`, one `ed25519-v1 <key> <label>` a line.
    pub fn read(text: &str) -> Self {
        let Roll { keys, skipped, .. } = Roll::read(text);
        for line in &skipped {
            logging::warn!(name: logging::event::OPERATOR_SKIPPED, error = logging::Failure(line).field());
        }
        Self { keys, skipped }
    }

    /// The lines that were not keys.
    pub fn skipped(&self) -> &[OperatorError] {
        &self.skipped
    }
}

impl Operators for Pinned {
    type Error = Infallible;

    fn keys(&self) -> Result<Vec<OperatorKey>, Infallible> {
        Ok(self.keys.clone())
    }
}
