//! What can go wrong: loading a component ([`Error`], an operator's problem), and one call into
//! it ([`Failure`], which becomes a diagnostic, never a panic).

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use model::{Bytes, Chain, Diagnostic, Problem};

use crate::mirror::{Malformed, Unmirrored};

/// A component that could not be loaded.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// wasmtime refused the engine's settings.
    Engine {
        /// Why.
        source: wasmtime::Error,
    },
    /// The bytes are no component wasmtime can compile.
    Compile {
        /// The plugin.
        plugin: String,
        /// Why.
        source: wasmtime::Error,
    },
    /// The component imports something the host does not give, such as a WASI interface.
    Link {
        /// The plugin.
        plugin: String,
        /// Why.
        source: wasmtime::Error,
    },
    /// The `.cwasm` cache could not be read or written.
    Cache {
        /// The file.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// The budget's epoch tick is zero, so no time budget could be counted.
    Tick {
        /// The tick given.
        tick: Duration,
    },
    /// A source's terms count time in another tick than the runtime's epoch advances in, so its
    /// deadline would be wrong: the terms must carry the runtime's tick.
    TermsTick {
        /// The tick the terms gave.
        terms: Duration,
        /// The tick of the runtime.
        runtime: Duration,
    },
    /// The component's first call, asking its name, failed.
    Named {
        /// Why.
        failure: Failure,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine { .. } => out.write_str("wasmtime refused the plugin engine's settings"),
            Self::Compile { plugin, .. } => write!(out, "the plugin `{plugin}` is no component wasmtime compiles"),
            Self::Link { plugin, .. } => write!(out, "the plugin `{plugin}` imports what the host does not give"),
            Self::Cache { path, .. } => write!(out, "the compiled plugin cache {} could not be used", path.display()),
            Self::Tick { tick } => write!(out, "the epoch tick of {tick:?} cannot count a time budget"),
            Self::TermsTick { terms, runtime } => {
                write!(out, "the source's terms tick {terms:?} is not the runtime's epoch tick {runtime:?}")
            }
            Self::Named { .. } => out.write_str("the theme did not say its name"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine { source } | Self::Compile { source, .. } | Self::Link { source, .. } => Some(source.as_ref()),
            Self::Cache { source, .. } => Some(source),
            Self::Tick { .. } | Self::TermsTick { .. } => None,
            Self::Named { failure } => Some(failure),
        }
    }
}

/// One call into a plugin that went wrong.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failure {
    /// It ran out of fuel.
    Fuel {
        /// Its budget.
        limit: u64,
    },
    /// It ran past its time.
    Time {
        /// Its budget, in milliseconds.
        limit: u64,
    },
    /// It asked for more memory than it may have.
    Memory {
        /// Its budget.
        limit: Bytes,
        /// The size it asked to grow to.
        wanted: Bytes,
    },
    /// It asked for a table larger than it may have.
    Table {
        /// Its budget, in elements.
        limit: u32,
        /// The size it asked to grow to, in elements.
        wanted: u64,
    },
    /// It trapped: a panic, a stack overflow, an unreachable instruction.
    Trapped {
        /// The trap, as wasmtime says it.
        source: wasmtime::Error,
    },
    /// It gave back a value the model refuses.
    Malformed {
        /// Why.
        source: Malformed,
    },
    /// The host had a value for it that the boundary cannot carry.
    Unmirrored {
        /// Why.
        source: Unmirrored,
    },
    /// The instance a call needed had ended: a bug in the host.
    Ended,
}

impl Failure {
    /// The diagnostic saying it, for the plugin `plugin`.
    pub fn diagnostic(&self, plugin: &str) -> Diagnostic {
        let plugin = plugin.to_owned();
        Diagnostic::new(match self {
            Self::Fuel { limit } => Problem::PluginBudgetFuel { plugin, limit: *limit },
            Self::Time { limit } => Problem::PluginBudgetTime { plugin, limit: *limit },
            Self::Memory { limit, .. } => Problem::PluginBudgetMemory { plugin, limit: *limit },
            Self::Trapped { source } => Problem::PluginCallTrapped { plugin, reason: Trace(source).to_string() },
            Self::Malformed { source } => {
                Problem::PluginValueMalformed { plugin, reason: Chain::new(source).to_string() }
            }
            Self::Unmirrored { source } => Problem::PluginValueUnmirrored { reason: source.to_string() },
            Self::Table { limit, .. } => Problem::PluginBudgetTable { plugin, limit: u64::from(*limit) },
            Self::Ended => Problem::PluginCallTrapped { plugin, reason: self.to_string() },
        })
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fuel { limit } => write!(out, "the plugin ran past its {limit} units of fuel"),
            Self::Time { limit } => write!(out, "the plugin ran past its {limit} ms"),
            Self::Memory { limit, wanted } => {
                write!(out, "the plugin asked for {wanted} of memory, more than its {limit}")
            }
            Self::Table { limit, wanted } => {
                write!(out, "the plugin asked for a table of {wanted} elements, more than its {limit}")
            }
            Self::Trapped { .. } => out.write_str("the plugin trapped"),
            Self::Malformed { .. } => out.write_str("the plugin gave back a value the model refuses"),
            Self::Unmirrored { .. } => out.write_str("a value has no case at the plugin boundary"),
            Self::Ended => out.write_str("the plugin's instance had ended"),
        }
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Trapped { source } => Some(source.as_ref()),
            Self::Malformed { source } => Some(source),
            Self::Unmirrored { source } => Some(source),
            Self::Fuel { .. } | Self::Time { .. } | Self::Memory { .. } | Self::Table { .. } | Self::Ended => None,
        }
    }
}

/// A wasmtime error and its causes as one line.
struct Trace<'e>(&'e wasmtime::Error);

impl fmt::Display for Trace<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for error in self.0.chain() {
            if !first {
                out.write_str(": ")?;
            }
            write!(out, "{error}")?;
            first = false;
        }
        Ok(())
    }
}
