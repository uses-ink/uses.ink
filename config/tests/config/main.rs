//! The config crate through its public API: the fixtures under `fixture/`, and the round trip
//! of every code through `pins` and `Config::parse`.

#![allow(clippy::unwrap_used, unreachable_pub, reason = "tests stop at the first surprise")]

mod fixture;
mod round_trip;

use model::{AnyDecision, Code, Each};
use uses_ink_config::Config;

/// Whether `config` pins the value `decision` decided, on its subject.
struct Pins<'a> {
    config: &'a Config,
    decision: &'a AnyDecision,
}

impl<C: Code> Each<C> for Pins<'_> {
    type Out = bool;

    fn each(&mut self) -> bool {
        let decision = self.decision.get::<C>().unwrap();
        self.config.pin::<C>(&decision.subject).is_some_and(|pin| pin.value == decision.value)
    }
}
