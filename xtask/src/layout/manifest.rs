//! The rules over what the crates' manifests say: names, dependency order, licences, aliases
//! (docs/conventions.md, Crates and directories, and Licence).

use super::workspace::{Crate, Kind, Workspace};
use super::{Problem, Problems, Rule};

/// Crates with a name of their own: product-independent, published as is, MIT OR Apache-2.0,
/// depending on no crate of ours.
const OWN: &[&str] = &["freehand", "freehand-web"];

/// The repo's task runner: named as cargo expects, not published.
const TASKS: &str = "xtask";

/// A crate's name is its path: `plugin/theme/quiet` is `uses-ink-theme-quiet`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Names;

impl Names {
    /// The name `krate`'s path predicts.
    fn expected(krate: &Crate) -> String {
        let path = krate.place();
        if krate.name == TASKS || OWN.contains(&krate.name.as_str()) {
            path.rsplit('/').next().unwrap_or(&path).to_owned()
        } else {
            // the slot directory drops
            format!("uses-ink-{}", path.strip_prefix("plugin/").unwrap_or(&path).replace('/', "-"))
        }
    }
}

impl Rule for Names {
    const NAME: &'static str = "names";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for krate in &workspace.crates {
            let expected = Self::expected(krate);
            if krate.name != expected {
                problems.push(Problem::Name { krate: krate.name.clone(), path: krate.place(), expected });
            }
        }
    }
}

/// Dependencies flow one way (architecture.md section 4).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Order;

impl Order {
    /// The fixture harness: every crate may take it as a dev-dependency, none as a normal one.
    pub(crate) const FIXTURE: &'static str = "uses-ink-fixture";

    /// Which crates of ours each crate may depend on. A crate missing here is a problem: its
    /// place in the order is decided when it is added.
    const MAY_USE: &'static [(&'static str, &'static [&'static str])] = &[
        ("freehand", &[]),
        ("freehand-web", &["freehand"]),
        ("uses-ink-model", &[]),
        ("uses-ink-config", &["uses-ink-model"]),
        ("uses-ink-diag", &["uses-ink-model"]),
        ("uses-ink-engine", &["uses-ink-model", "uses-ink-diag", "uses-ink-config"]),
        ("uses-ink-token", &[]),
        // Formats speak the model directly until host/wasmtime exists, then the generated WIT types.
        ("uses-ink-format-markdown", &["uses-ink-model"]),
        ("uses-ink-host-native", &["uses-ink-model", "uses-ink-engine"]),
        // The Theme trait, until themes are components speaking the generated WIT types.
        ("uses-ink-theme-quiet", &["uses-ink-model", "uses-ink-engine"]),
        (
            "uses-ink-cli",
            &[
                "uses-ink-model",
                "uses-ink-config",
                "uses-ink-diag",
                "uses-ink-engine",
                "uses-ink-host-native",
                "uses-ink-format-markdown",
                "uses-ink-theme-quiet",
            ],
        ),
        (TASKS, &["freehand", "uses-ink-token"]),
        (Self::FIXTURE, &[]),
    ];
}

impl Rule for Order {
    const NAME: &'static str = "order";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let ours: Vec<&str> = workspace.crates.iter().map(|c| c.name.as_str()).collect();
        for krate in &workspace.crates {
            let Some(&(_, allowed)) = Self::MAY_USE.iter().find(|(name, _)| *name == krate.name) else {
                problems.push(Problem::Unordered { krate: krate.name.clone() });
                continue;
            };
            for dependency in &krate.dependencies {
                if dependency.name == Self::FIXTURE {
                    if dependency.kind != Some(Kind::Dev) {
                        problems.push(Problem::Fixture { krate: krate.name.clone() });
                    }
                    continue;
                }
                let name = dependency.name.as_str();
                if dependency.inside && ours.contains(&name) && !allowed.contains(&name) {
                    problems.push(Problem::Dependency { krate: krate.name.clone(), on: dependency.name.clone() });
                }
            }
        }
    }
}

/// A crate's licence matches its kind: permissive for a crate with a name of its own, the
/// service's otherwise.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Licence;

impl Licence {
    const PERMISSIVE: &'static str = "MIT OR Apache-2.0";
    const SERVICE: &'static str = "AGPL-3.0-or-later";
}

impl Rule for Licence {
    const NAME: &'static str = "licence";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for krate in &workspace.crates {
            let expected = if OWN.contains(&krate.name.as_str()) { Self::PERMISSIVE } else { Self::SERVICE };
            if krate.licence.as_deref() != Some(expected) {
                problems.push(Problem::Licence { krate: krate.name.clone(), found: krate.licence.clone(), expected });
            }
        }
    }
}

/// A crate another member uses is named through the workspace's alias.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Alias;

impl Rule for Alias {
    const NAME: &'static str = "alias";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let used: Vec<&str> =
            workspace.crates.iter().flat_map(|c| c.dependencies.iter().map(|d| d.name.as_str())).collect();
        for krate in &workspace.crates {
            if used.contains(&krate.name.as_str()) && !workspace.aliased.contains(&krate.name) {
                problems.push(Problem::Alias { krate: krate.name.clone() });
            }
        }
    }
}
