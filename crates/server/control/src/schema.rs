//! The control plane's migrations in the order they apply, so a harness, a server and a test
//! name one list and a new package adds one line here.

use db::{StepError, Steps};

use crate::{Held, keys, passkeys, recovery, signin};

/// What the control plane adds to a database after the queue's migration (number 100): the
/// sign-in's, the keys', the passkeys', the recovery codes' (numbers 200 to 203) and the
/// packages of milestone M4 (numbers 301 and up).
#[derive(Clone, Copy, Debug)]
pub struct Schema;

impl Schema {
    /// `steps`, which hold the migrations up to the queue's, with every later migration of the
    /// control plane added.
    pub fn after(steps: Steps) -> Result<Steps, StepError> {
        steps
            .with::<signin::Entry>()?
            .with::<keys::Entry>()?
            .with::<passkeys::Entry>()?
            .with::<recovery::Entry>()?
            .with::<Held>()
    }
}
