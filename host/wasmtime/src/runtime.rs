//! The wasmtime engine every component of a build runs on: its settings (fuel, epochs), the
//! budget of each call, the cache, the epoch's ticker and the host's grammars.

use std::sync::Arc;

use host_native::Grammars;
use wasmtime::component::Component as Compiled;
use wasmtime::{Store, Trap};

use crate::budget::{Budget, Ticker};
use crate::cache::{Cache, Compile, Origin};
use crate::capability::State;
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
        let ticker = Ticker::start(engine.clone());
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
        match &self.cache {
            Some(cache) => cache.load(&self.engine, plugin, bytes),
            None => Ok((Compile(plugin).run(&self.engine, bytes)?, Origin::Compiled)),
        }
    }

    /// A fresh store for one instance of `plugin`, its memory limited.
    pub(crate) fn store(&self, plugin: &str) -> Store<State> {
        let mut store = Store::new(&self.engine, State::new(plugin, self.grammars.clone(), &self.budget));
        store.limiter(|state| &mut state.limit);
        store
    }

    /// Runs `call` on `store` with a fresh budget, a trap or a limit hit as its [`Failure`].
    pub(crate) fn call<T>(
        &self,
        store: &mut Store<State>,
        call: impl FnOnce(&mut Store<State>) -> wasmtime::Result<T>,
    ) -> Result<T, Failure> {
        let budget = &self.budget;
        let fueled = store.set_fuel(budget.fuel);
        store.set_epoch_deadline(budget.ticks());
        store.epoch_deadline_trap();
        let result = fueled.and_then(|()| call(store));
        result.map_err(|source| {
            if store.data().limit.hit {
                return Failure::Memory { limit: budget.memory };
            }
            match source.downcast_ref::<Trap>() {
                Some(Trap::OutOfFuel) => Failure::Fuel { limit: budget.fuel },
                Some(Trap::Interrupt) => {
                    Failure::Time { limit: u64::try_from(budget.time.as_millis()).unwrap_or(u64::MAX) }
                }
                _ => Failure::Trapped { source },
            }
        })
    }
}
