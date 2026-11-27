//! The wasmtime engine every component of a build runs on: its settings (fuel, epochs), the
//! budget of each call, the cache, the epoch's ticker and the host's grammars.

use std::sync::Arc;
use std::time::Instant;

use trace::Field as _;

use host_native::Grammars;
use model::Bytes;
use wasmtime::component::Component as Compiled;
use wasmtime::{Store, Trap};

use crate::budget::{Budget, Ticker};
use crate::cache::{Cache, Compile, Origin};
use crate::capability::{Refusal, Sourcing, State};
use crate::error::{Error, Failure};

/// Loads plugins as components and runs their calls within a [`Budget`].
#[derive(Clone, Debug)]
pub struct Runtime {
    pub(crate) shared: Arc<Shared>,
}

/// What every component of a runtime shares.
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) engine: wasmtime::Engine,
    pub(crate) budget: Budget,
    cache: Option<Cache>,
    grammars: Grammars,
    _ticker: Ticker,
}

impl Runtime {
    /// A runtime whose calls each get `budget`, keeping compiled components in `cache` when
    /// one is given.
    pub fn new(budget: Budget, cache: Option<Cache>) -> Result<Self, Error> {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true).epoch_interruption(true);
        let engine = wasmtime::Engine::new(&config).map_err(|source| Error::Engine { source })?;
        if budget.ticks().is_none() {
            return Err(Error::Tick { tick: budget.tick });
        }
        let ticker = Ticker::start(engine.clone(), budget.tick);
        let shared = Shared { engine, budget, cache, grammars: Grammars::default(), _ticker: ticker };
        Ok(Self { shared: Arc::new(shared) })
    }

    /// The budget of each call.
    pub fn budget(&self) -> Budget {
        self.shared.budget
    }
}

impl Shared {
    /// The component in `bytes`, compiled or from the cache.
    pub(crate) fn compile(&self, plugin: &str, bytes: &[u8]) -> Result<(Compiled, Origin), Error> {
        let loaded = match &self.cache {
            Some(cache) => cache.load(&self.engine, plugin, bytes),
            None => Ok((Compile(plugin).run(&self.engine, bytes)?, Origin::Compiled)),
        };
        if let Ok((_, origin)) = &loaded {
            trace::debug!(
                name: trace::event::PLUGIN_LOADED,
                plugin = trace::Shown(plugin).field(),
                origin = match origin {
                    Origin::Compiled => "compiled",
                    Origin::Cached => "cached",
                    _ => "replaced",
                },
                bytes = trace::Size::from(bytes.len()).field(),
            );
        }
        loaded
    }

    /// A fresh store for one instance of `plugin`, its memory limited.
    pub(crate) fn store(&self, plugin: &str) -> Store<State> {
        self.store_in(plugin, &self.budget, Sourcing::default())
    }

    /// A fresh store for one instance of `plugin`, limited by `budget` and fetching as
    /// `sourcing` says.
    pub(crate) fn store_in(&self, plugin: &str, budget: &Budget, sourcing: Sourcing) -> Store<State> {
        let state = State::new(plugin, self.grammars.clone(), budget).sourcing(sourcing);
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limit);
        store
    }

    /// Runs `call` on `store` with a fresh budget, a trap or a limit hit as its [`Failure`].
    pub(crate) fn call<T>(
        &self,
        store: &mut Store<State>,
        call: impl FnOnce(&mut Store<State>) -> wasmtime::Result<T>,
    ) -> Result<T, Failure> {
        Self::call_in(&self.budget, store, call)
    }

    /// Like [`Shared::call`], with `budget` in place of the runtime's: a source's fetch takes
    /// longer than a page's render.
    /// Records on a call's span what it used: fuel, and how long it took.
    fn measured(span: &trace::Span, started: Instant, fuel: u64) {
        span.record("fuel", trace::Count(fuel).field());
        span.record("took_us", trace::Took(started.elapsed()).field());
    }

    pub(crate) fn call_in<T>(
        budget: &Budget,
        store: &mut Store<State>,
        call: impl FnOnce(&mut Store<State>) -> wasmtime::Result<T>,
    ) -> Result<T, Failure> {
        // A refusal belongs to the call that met it: one a plugin handled on an earlier call
        // (a theme's session lives across pages) must not name a later trap.
        store.data_mut().limit.refused = None;
        let span = trace::plugin!(store.data().plugin());
        let _entered = span.clone().entered();
        // the clock is read only when something listens
        let started = (!span.is_disabled()).then(Instant::now);
        let fueled = store.set_fuel(budget.fuel);
        // Never zero: a runtime refuses a zero tick, and a source's terms must carry its tick.
        store.set_epoch_deadline(budget.ticks().unwrap_or(1));
        store.epoch_deadline_trap();
        let result = fueled.and_then(|()| call(store));
        if let Some(started) = started {
            Self::measured(&span, started, budget.fuel.saturating_sub(store.get_fuel().unwrap_or(budget.fuel)));
        }
        let result = result.map_err(|source| {
            let bytes = |wanted| Bytes(u64::try_from(wanted).unwrap_or(u64::MAX));
            // Fuel and time end a call whatever a growth refusal said before them.
            match source.downcast_ref::<Trap>() {
                Some(Trap::OutOfFuel) => return Failure::Fuel { limit: budget.fuel },
                Some(Trap::Interrupt) => {
                    return Failure::Time { limit: u64::try_from(budget.time.as_millis()).unwrap_or(u64::MAX) };
                }
                _ => {}
            }
            match store.data_mut().limit.refused.take() {
                Some(Refusal::Memory { wanted }) => {
                    return Failure::Memory { limit: budget.memory, wanted: bytes(wanted) };
                }
                Some(Refusal::Table { wanted }) => {
                    return Failure::Table { limit: budget.table, wanted: u64::try_from(wanted).unwrap_or(u64::MAX) };
                }
                None => {}
            }
            Failure::Trapped { source }
        });
        if let Err(failure) = &result {
            trace::warn!(
                name: trace::event::PLUGIN_REFUSED,
                plugin = trace::Shown(store.data().plugin()).field(),
                error = trace::Failure(failure).field(),
            );
        }
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn failure(
        setup: impl FnOnce(&mut Store<State>),
        call: impl FnOnce(&mut Store<State>) -> wasmtime::Result<()>,
    ) -> Failure {
        let runtime = Runtime::new(Budget::default(), None).map_err(|error| error.to_string());
        let Ok(runtime) = runtime else { return Failure::Ended };
        let mut store = runtime.shared.store("test");
        setup(&mut store);
        runtime.shared.call(&mut store, call).err().unwrap_or(Failure::Ended)
    }

    #[test]
    fn a_refusal_of_an_earlier_call_does_not_name_a_later_trap() {
        let stale = |store: &mut Store<State>| store.data_mut().limit.refused = Some(Refusal::Memory { wanted: 1 });
        let failed = failure(stale, |_| Err(Trap::OutOfFuel.into()));
        assert!(matches!(failed, Failure::Fuel { .. }), "{failed:?}");
        let failed = failure(stale, |_| Err(Trap::UnreachableCodeReached.into()));
        assert!(matches!(failed, Failure::Trapped { .. }), "{failed:?}");
    }

    #[test]
    fn a_handled_refusal_does_not_hide_the_fuel_that_ended_the_call() {
        let failed = failure(
            |_| {},
            |store| {
                store.data_mut().limit.refused = Some(Refusal::Table { wanted: 9 });
                Err(Trap::OutOfFuel.into())
            },
        );
        assert!(matches!(failed, Failure::Fuel { .. }), "{failed:?}");
    }
}
