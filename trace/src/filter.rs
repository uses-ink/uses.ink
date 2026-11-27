//! The filter language: `ink=debug,edge=trace,control=info`.
//!
//! A directive names an [`Area`] and a level. An area is a part of the product, spelled the way
//! a person says it, and stands for the targets (module paths) of the crates that make it up.
//! A name that is no area is taken as a target as written, so `wasmtime=warn` and a full module
//! path work too. A bare level (`debug`) is the level of `ink`. The most specific directive
//! wins, as in any `tracing` filter.

use std::fmt;
use std::str::FromStr;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::Targets;

/// A part of the product.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Area {
    /// Everything of ours.
    Ink,
    /// The engine: interpret, render, the unit planner.
    Engine,
    /// Building: the builders and the folder and git flows.
    Build,
    /// Serving: the edge, its adapters and the API.
    Edge,
    /// The control plane.
    Control,
    /// The database port and its adapters.
    Db,
    /// The blob store.
    Blob,
    /// The hosts: the image pipeline, the native host and the wasm host.
    Host,
    /// The plugins.
    Plugin,
    /// The watch and the worker.
    Watch,
    /// The queue.
    Queue,
    /// The command line.
    Cli,
    /// Anything else, as a target prefix.
    Target(String),
}

impl Area {
    /// The target prefixes the area stands for. Module paths use `_`, crate names `-`.
    pub fn prefixes(&self) -> Vec<String> {
        let ours = |tails: &[&str]| tails.iter().map(|tail| format!("uses_ink_{tail}")).collect();
        match self {
            Self::Ink => vec!["uses_ink".to_owned()],
            Self::Engine => ours(&["engine"]),
            Self::Build => ours(&["build"]),
            Self::Edge => ours(&["edge"]),
            Self::Control => ours(&["control"]),
            Self::Db => ours(&["db"]),
            Self::Blob => ours(&["blob"]),
            Self::Host => ours(&["host"]),
            Self::Plugin => ours(&["format", "theme", "source", "sdk"]),
            Self::Watch => ours(&["watch"]),
            Self::Queue => ours(&["queue"]),
            Self::Cli => ours(&["cli"]),
            Self::Target(target) => vec![target.replace('-', "_")],
        }
    }
}

impl FromStr for Area {
    type Err = std::convert::Infallible;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "ink" => Self::Ink,
            "engine" => Self::Engine,
            "build" => Self::Build,
            "edge" => Self::Edge,
            "control" => Self::Control,
            "db" => Self::Db,
            "blob" => Self::Blob,
            "host" => Self::Host,
            "plugin" => Self::Plugin,
            "watch" => Self::Watch,
            "queue" => Self::Queue,
            "cli" => Self::Cli,
            other => Self::Target(other.to_owned()),
        })
    }
}

impl fmt::Display for Area {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ink => out.write_str("ink"),
            Self::Engine => out.write_str("engine"),
            Self::Build => out.write_str("build"),
            Self::Edge => out.write_str("edge"),
            Self::Control => out.write_str("control"),
            Self::Db => out.write_str("db"),
            Self::Blob => out.write_str("blob"),
            Self::Host => out.write_str("host"),
            Self::Plugin => out.write_str("plugin"),
            Self::Watch => out.write_str("watch"),
            Self::Queue => out.write_str("queue"),
            Self::Cli => out.write_str("cli"),
            Self::Target(target) => out.write_str(target),
        }
    }
}

/// Why a filter did not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FilterError {
    /// A level that is none of `off`, `error`, `warn`, `info`, `debug`, `trace`.
    Level {
        /// The directive it was in.
        directive: String,
        /// The word that is no level.
        word: String,
    },
    /// A directive with no area before the `=`.
    Area {
        /// The directive.
        directive: String,
    },
}

impl fmt::Display for FilterError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Level { directive, word } => write!(
                out,
                "`{word}` in `{directive}` is not a level: use off, error, warn, info, debug or trace"
            ),
            Self::Area { directive } => write!(out, "`{directive}` names no area before the `=`"),
        }
    }
}

impl std::error::Error for FilterError {}

/// One directive: an area at a level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directive {
    /// Where it applies.
    pub area: Area,
    /// Down to which level.
    pub level: LevelFilter,
}

/// A parsed filter: directives in the order written, a later one for the same area replacing an
/// earlier one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    directives: Vec<Directive>,
}

impl Filter {
    /// A filter of one area at one level.
    pub fn of(area: Area, level: LevelFilter) -> Self {
        Self { directives: vec![Directive { area, level }] }
    }

    /// Whether it says nothing.
    pub fn is_empty(&self) -> bool {
        self.directives.is_empty()
    }

    /// The directives.
    pub fn directives(&self) -> &[Directive] {
        &self.directives
    }

    /// This filter, with `later` after it: where both name an area, `later` wins.
    #[must_use]
    pub fn then(mut self, later: Self) -> Self {
        for directive in later.directives {
            self.directives.retain(|known| known.area != directive.area);
            self.directives.push(directive);
        }
        self
    }

    /// The `tracing` filter this stands for.
    pub fn targets(&self) -> Targets {
        let mut targets = Targets::new();
        for directive in &self.directives {
            for prefix in directive.area.prefixes() {
                targets = targets.with_target(prefix, directive.level);
            }
        }
        targets
    }

    fn level(word: &str, directive: &str) -> Result<LevelFilter, FilterError> {
        word.parse().map_err(|_| FilterError::Level { directive: directive.to_owned(), word: word.to_owned() })
    }
}

impl FromStr for Filter {
    type Err = FilterError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut filter = Self::default();
        for directive in text.split(',').map(str::trim).filter(|part| !part.is_empty()) {
            let parsed = match directive.split_once('=') {
                Some((area, level)) => {
                    if area.trim().is_empty() {
                        return Err(FilterError::Area { directive: directive.to_owned() });
                    }
                    Self::of(area.trim().parse().unwrap_or(Area::Ink), Self::level(level.trim(), directive)?)
                }
                // a bare level is the level of everything of ours; a bare word that is no level is
                // an area at its most verbose
                None => match directive.parse::<LevelFilter>() {
                    Ok(level) => Self::of(Area::Ink, level),
                    Err(_) => Self::of(directive.parse().unwrap_or(Area::Ink), LevelFilter::TRACE),
                },
            };
            filter = filter.then(parsed);
        }
        Ok(filter)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn areas_levels_and_a_bare_level_parse() {
        let filter: Filter = "ink=info, edge=trace,control=debug".parse().unwrap();
        let areas: Vec<_> = filter.directives().iter().map(|d| (d.area.to_string(), d.level)).collect();
        assert_eq!(
            areas,
            [("ink".into(), LevelFilter::INFO), ("edge".into(), LevelFilter::TRACE), ("control".into(), LevelFilter::DEBUG)]
        );
        assert_eq!("debug".parse::<Filter>().unwrap(), Filter::of(Area::Ink, LevelFilter::DEBUG));
        assert_eq!("edge".parse::<Filter>().unwrap(), Filter::of(Area::Edge, LevelFilter::TRACE));
    }

    #[test]
    fn a_later_directive_replaces_an_earlier_one_for_the_same_area() {
        let filter: Filter = "edge=info,edge=trace".parse().unwrap();
        assert_eq!(filter, Filter::of(Area::Edge, LevelFilter::TRACE));
    }

    #[test]
    fn a_bad_level_and_a_missing_area_are_typed_errors() {
        assert!(matches!("edge=loud".parse::<Filter>(), Err(FilterError::Level { .. })));
        assert!(matches!("=info".parse::<Filter>(), Err(FilterError::Area { .. })));
    }

    #[test]
    fn an_area_stands_for_the_targets_of_its_crates() {
        let targets = "edge=debug".parse::<Filter>().unwrap().targets();
        assert!(targets.would_enable("uses_ink_edge_axum::server", &tracing::Level::DEBUG));
        assert!(!targets.would_enable("uses_ink_engine", &tracing::Level::ERROR));
    }
}
