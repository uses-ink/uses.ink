//! The checks every forge's delivery passes, in the one order.

use jiff::Timestamp;

use crate::{Delivery, Limits, Refused, Replay};

/// The size limit and the replay window of one forge's webhook.
#[derive(Debug)]
pub struct Intake {
    limits: Limits,
    replay: Replay,
}

impl Intake {
    /// An intake under `limits`.
    pub fn new(limits: Limits) -> Self {
        Self { limits, replay: Replay::new(limits.window, limits.remembered) }
    }

    /// The limits.
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Refuses a body larger than the setting, before anything reads it.
    pub fn size(&self, body: &[u8]) -> Result<(), Refused> {
        if body.len() > self.limits.body {
            return Err(Refused::Oversize { size: body.len(), limit: self.limits.body });
        }
        Ok(())
    }

    /// Reads the delivery id, and refuses it when it was seen inside the window. Call it after
    /// the signature held, so only a sender who holds the secret takes room in the memory.
    pub fn admit(&self, id: &str, now: Timestamp) -> Result<Delivery, Refused> {
        let delivery: Delivery = id.parse().map_err(Refused::Delivery)?;
        self.replay.admit(&delivery, now)?;
        Ok(delivery)
    }
}
