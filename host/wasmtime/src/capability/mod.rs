//! The host's capabilities, implemented once and linked into every component (architecture.md
//! section 6): the pen through `freehand`, text measuring, highlighting through host-native's
//! [`Grammars`], and the record of what a plugin says. Each call's store holds a [`State`].

mod diagnostic;
mod git;
mod highlight;
mod pen;
mod text;

use engine::{Record as _, Said};
use host_native::Grammars;
use model::{Diagnostic, Problem};
use wasmtime::ResourceLimiter;

use crate::bind::plugin::ink::model as types;
use crate::budget::Budget;

pub(crate) use git::Sourcing;

/// What one store holds: the plugin's name, the grammars, what the plugin said, and its memory
/// limit.
#[derive(Debug)]
pub(crate) struct State {
    plugin: String,
    grammars: Grammars,
    pub(crate) said: Said,
    pub(crate) limit: Limit,
    pub(crate) sourcing: Sourcing,
}

impl State {
    pub(crate) fn new(plugin: impl Into<String>, grammars: Grammars, budget: &Budget) -> Self {
        let limit = Limit { memory: budget.bytes(), table: budget.table, refused: None };
        Self { plugin: plugin.into(), grammars, said: Said::default(), limit, sourcing: Sourcing::default() }
    }

    /// The plugin's name, as its spans and events carry it.
    pub(crate) fn plugin(&self) -> &str {
        &self.plugin
    }

    /// This state with `sourcing` as what its `git` calls use.
    pub(crate) fn sourcing(mut self, sourcing: Sourcing) -> Self {
        self.sourcing = sourcing;
        self
    }

    /// `plugin.value.malformed`: the plugin sent something the model refuses.
    fn malformed(&mut self, refused: &impl std::error::Error) {
        let problem = Problem::PluginValueMalformed { plugin: self.plugin.clone(), reason: refused.to_string() };
        self.said.diagnose(Diagnostic::new(problem));
    }
}

/// The memory and table an instance may grow to, and the first growth it was refused.
#[derive(Debug)]
pub(crate) struct Limit {
    memory: usize,
    table: u32,
    pub(crate) refused: Option<Refusal>,
}

/// A growth the limiter refused, with the size asked for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Refusal {
    Memory { wanted: usize },
    Table { wanted: usize },
}

impl ResourceLimiter for Limit {
    fn memory_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> wasmtime::Result<bool> {
        let allowed = desired <= self.memory;
        if !allowed {
            self.refused.get_or_insert(Refusal::Memory { wanted: desired });
        }
        Ok(allowed)
    }

    fn table_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> wasmtime::Result<bool> {
        let allowed = u64::try_from(desired).is_ok_and(|desired| desired <= u64::from(self.table));
        if !allowed {
            self.refused.get_or_insert(Refusal::Table { wanted: desired });
        }
        Ok(allowed)
    }
}

impl types::diagnostic::Host for State {}
impl types::document::Host for State {}
impl types::site::Host for State {}
impl types::read::Host for State {}
impl types::file::Host for State {}
impl types::snapshot::Host for State {}
