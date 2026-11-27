//! The rules over what the crates' manifests say: names, dependency order, licences, aliases
//! (docs/conventions.md, Crates and directories, and Licence).

use super::workspace::{Crate, Kind, Workspace};
use super::{Problem, Problems, Rule};

/// Crates with a name of their own: product-independent, published as is, MIT OR Apache-2.0,
/// depending on no crate of ours.
const OWN: &[&str] = &["freehand", "freehand-web"];

/// The repo's task runner: named as cargo expects, not published.
const TASKS: &str = "xtask";

/// The build farm client (`tools/farm`): installed as `farm`, so named as it is run, depending on
/// no crate of ours.
const FARM: &str = "farm";

/// A crate's name is its path: `plugin/theme/quiet` is `uses-ink-theme-quiet`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Names;

impl Names {
    /// The name `krate`'s path predicts.
    fn expected(krate: &Crate) -> String {
        let path = krate.place();
        if krate.name == TASKS || krate.name == FARM || OWN.contains(&krate.name.as_str()) {
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
        // Serving a deploy (decision 0051): the edge speaks the model alone; its adapters add the
        // runtime (axum) or a folder of deploys, never each other.
        ("uses-ink-edge", &["uses-ink-model"]),
        ("uses-ink-edge-axum", &["uses-ink-model", "uses-ink-edge", "uses-ink-edge-api"]),
        ("uses-ink-edge-folder", &["uses-ink-model", "uses-ink-edge"]),
        ("uses-ink-token", &[]),
        // The API and the polling watch (decision 0052) read time and randomness through the
        // clock's ports and know the control plane; the adapters they run on are dev-only here.
        ("uses-ink-clock", &[]),
        (
            "uses-ink-edge-api",
            &[
                "uses-ink-blob",
                "uses-ink-blob-dir",
                "uses-ink-build",
                "uses-ink-clock",
                "uses-ink-control",
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-edge-folder",
                "uses-ink-mail",
                "uses-ink-model",
                "uses-ink-queue-local",
                "uses-ink-secret-local",
            ],
        ),
        // The edge's host-to-deploy port over the control plane (decision 0067): the edge's port
        // traits, the folder adapter's reads and the control plane, over the clock's ports.
        (
            "uses-ink-edge-control",
            &[
                "uses-ink-blob",
                "uses-ink-blob-dir",
                "uses-ink-build",
                "uses-ink-clock",
                "uses-ink-control",
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-edge",
                "uses-ink-edge-folder",
                "uses-ink-model",
                "uses-ink-queue-local",
                "uses-ink-secret-local",
            ],
        ),
        // The watch's Heads port over git's protocol (decision 0067): the watch's trait, git
        // through gix, the build port's revision and the model's locator.
        ("uses-ink-watch-git", &["uses-ink-build", "uses-ink-model", "uses-ink-secret", "uses-ink-watch"]),
        (
            "uses-ink-watch",
            &[
                "uses-ink-blob-dir",
                "uses-ink-build",
                "uses-ink-clock",
                "uses-ink-control",
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-model",
                "uses-ink-queue",
                "uses-ink-queue-local",
                "uses-ink-secret",
                "uses-ink-secret-local",
            ],
        ),
        // A format's native core speaks the model; its guest crate speaks the WIT (0041).
        ("uses-ink-format-markdown", &["uses-ink-model"]),
        // A dialect of markdown reads through the markdown format's comrak setup.
        ("uses-ink-format-obsidian", &["uses-ink-model", "uses-ink-format-markdown"]),
        ("uses-ink-host-image", &["uses-ink-model", "uses-ink-engine"]),
        ("uses-ink-host-native", &["uses-ink-model", "uses-ink-engine"]),
        // A theme's native core implements the engine's Theme trait; its guest crate exports it.
        ("uses-ink-theme-quiet", &["uses-ink-model", "uses-ink-engine"]),
        (
            "uses-ink-cli",
            &[
                "uses-ink-model",
                "uses-ink-config",
                "uses-ink-diag",
                "uses-ink-engine",
                "uses-ink-edge",
                "uses-ink-edge-axum",
                "uses-ink-edge-folder",
                "uses-ink-edge-api",
                "uses-ink-edge-control",
                "uses-ink-clock",
                "uses-ink-control",
                "uses-ink-watch",
                "uses-ink-watch-git",
                "uses-ink-blob",
                "uses-ink-build",
                "uses-ink-build-local",
                "uses-ink-blob-dir",
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-queue-local",
                "uses-ink-secret-local",
                "uses-ink-mail",
                "uses-ink-mail-smtp",
                "uses-ink-secret",
                "uses-ink-host-image",
                "uses-ink-host-native",
                "uses-ink-format-markdown",
                "uses-ink-format-obsidian",
                "uses-ink-theme-quiet",
                "uses-ink-host-wasmtime",
            ],
        ),
        // The plugin boundary (decision 0035): the host runs components and tests them against
        // their native cores; the SDK and each guest speak the model through the shared WIT.
        (
            "uses-ink-host-wasmtime",
            &[
                "freehand",
                "uses-ink-model",
                "uses-ink-engine",
                "uses-ink-host-native",
                "uses-ink-host-image",
                "uses-ink-format-markdown",
                "uses-ink-format-obsidian",
                "uses-ink-theme-quiet",
                "uses-ink-source-git",
            ],
        ),
        // The git source's native core speaks the model's `Git` trait; its tests run it over the
        // host's own fetching.
        ("uses-ink-source-git", &["uses-ink-model", "uses-ink-host-native"]),
        ("uses-ink-source-git-guest", &["uses-ink-model", "uses-ink-sdk", "uses-ink-source-git"]),
        ("uses-ink-sdk", &["freehand", "uses-ink-model"]),
        ("uses-ink-format-markdown-guest", &["uses-ink-model", "uses-ink-sdk", "uses-ink-format-markdown"]),
        ("uses-ink-format-obsidian-guest", &["uses-ink-model", "uses-ink-sdk", "uses-ink-format-obsidian"]),
        ("uses-ink-theme-quiet-guest", &["uses-ink-model", "uses-ink-engine", "uses-ink-sdk", "uses-ink-theme-quiet"]),
        ("uses-ink-host-wasmtime-hostile", &["uses-ink-sdk"]),
        // The control plane (decision 0045): ports are crates that know the model and nothing of
        // an adapter; an adapter knows its port and its conformance suite, which is dev-only.
        ("uses-ink-blob", &["uses-ink-model"]),
        ("uses-ink-blob-conformance", &["uses-ink-blob", "uses-ink-model"]),
        ("uses-ink-blob-dir", &["uses-ink-blob", "uses-ink-blob-conformance", "uses-ink-model"]),
        ("uses-ink-build", &["uses-ink-blob", "uses-ink-model"]),
        ("uses-ink-build-conformance", &["uses-ink-blob", "uses-ink-build", "uses-ink-model"]),
        (
            "uses-ink-build-local",
            &[
                "uses-ink-blob",
                "uses-ink-blob-dir",
                "uses-ink-build",
                "uses-ink-build-conformance",
                "uses-ink-config",
                "uses-ink-engine",
                "uses-ink-format-markdown",
                "uses-ink-format-obsidian",
                "uses-ink-host-image",
                "uses-ink-host-native",
                "uses-ink-model",
                "uses-ink-theme-quiet",
            ],
        ),
        // The row port holds only typed statement builders (0049); its adapters and suite are the
        // only crates that know a driver.
        ("uses-ink-db", &[]),
        ("uses-ink-db-conformance", &["uses-ink-db"]),
        ("uses-ink-db-sqlite", &["uses-ink-db", "uses-ink-db-conformance"]),
        ("uses-ink-queue", &["uses-ink-model"]),
        ("uses-ink-queue-conformance", &["uses-ink-model", "uses-ink-queue"]),
        (
            "uses-ink-queue-local",
            &["uses-ink-db", "uses-ink-db-sqlite", "uses-ink-model", "uses-ink-queue", "uses-ink-queue-conformance"],
        ),
        // The mail port (decision 0066): typed messages and a trait, no adapter; the SMTP adapter
        // takes the port and the secret type that holds its password.
        ("uses-ink-mail", &[]),
        ("uses-ink-mail-smtp", &["uses-ink-mail", "uses-ink-secret"]),
        ("uses-ink-secret", &[]),
        ("uses-ink-secret-conformance", &["uses-ink-secret"]),
        ("uses-ink-secret-local", &["uses-ink-secret", "uses-ink-secret-conformance"]),
        // The use cases know the ports; the adapters are what its tests run against.
        (
            "uses-ink-control",
            &[
                "uses-ink-blob",
                "uses-ink-blob-dir",
                "uses-ink-build",
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-mail",
                "uses-ink-model",
                "uses-ink-queue",
                "uses-ink-queue-local",
                "uses-ink-secret",
                "uses-ink-secret-local",
            ],
        ),
        // The dashboard (decision 0053): typed views of the control plane's values, rendered with
        // `unspliced`; it knows no HTTP server and no adapter, the edge mounts it.
        (
            "uses-ink-web",
            &[
                "freehand",
                "uses-ink-build",
                "uses-ink-control",
                "uses-ink-diag",
                "uses-ink-engine",
                "uses-ink-model",
                "uses-ink-token",
            ],
        ),
        (TASKS, &["freehand", "uses-ink-token"]),
        (FARM, &[]),
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

/// A crate's licence matches its kind: permissive for a crate with a name of its own and for
/// what a third-party plugin links, the service's otherwise (decision 0026). A permissive crate
/// depends on no crate of ours under the service's licence, so linking it never pulls one in.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Licence;

impl Licence {
    pub(crate) const PERMISSIVE: &'static str = "MIT OR Apache-2.0";
    const SERVICE: &'static str = "AGPL-3.0-or-later";

    /// Crates of ours a third-party plugin links: the model it speaks and the SDK.
    const LINKED: &'static [&'static str] = &["uses-ink-model", "uses-ink-sdk"];

    /// Whether the crate named `name` must be permissive.
    fn is_permissive(name: &str) -> bool {
        OWN.contains(&name) || Self::LINKED.contains(&name)
    }
}

impl Rule for Licence {
    const NAME: &'static str = "licence";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let ours: Vec<&str> = workspace.crates.iter().map(|c| c.name.as_str()).collect();
        for krate in &workspace.crates {
            let permissive = Self::is_permissive(&krate.name);
            let expected = if permissive { Self::PERMISSIVE } else { Self::SERVICE };
            if krate.licence.as_deref() != Some(expected) {
                problems.push(Problem::Licence { krate: krate.name.clone(), found: krate.licence.clone(), expected });
            }
            if !permissive {
                continue;
            }
            // a dev-dependency is not linked by whoever uses the crate
            for dependency in krate.dependencies.iter().filter(|d| d.kind != Some(Kind::Dev)) {
                let name = dependency.name.as_str();
                if dependency.inside && ours.contains(&name) && !Self::is_permissive(name) {
                    problems.push(Problem::Copyleft { krate: krate.name.clone(), on: dependency.name.clone() });
                }
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::layout::workspace::{Dependency, Workspace};

    /// A crate of ours at `dir` with `licence` and path dependencies on `dependencies`, each
    /// `(name, kind)`.
    fn krate(name: &str, dir: &str, licence: Option<&str>, dependencies: &[(&str, Option<Kind>)]) -> Crate {
        Crate {
            name: name.to_owned(),
            dir: PathBuf::from(dir),
            licence: licence.map(str::to_owned),
            dependencies: dependencies
                .iter()
                .map(|&(name, kind)| Dependency { name: name.to_owned(), inside: true, kind })
                .collect(),
            files: vec![],
            modules: vec![],
        }
    }

    fn workspace(crates: Vec<Crate>, aliased: &[&str]) -> Workspace {
        Workspace {
            root: PathBuf::from("/w"),
            crates,
            aliased: aliased.iter().map(|name| (*name).to_owned()).collect(),
            entries: vec![],
        }
    }

    fn problems_of(rule: &impl Rule, workspace: &Workspace) -> Vec<Problem> {
        let mut problems = Problems::default();
        rule.check(workspace, &mut problems);
        problems.found
    }

    #[test]
    fn a_crate_is_named_for_its_path() {
        let named = |name, dir| problems_of(&Names, &workspace(vec![krate(name, dir, None, &[])], &[]));
        assert!(named("uses-ink-theme-quiet", "plugin/theme/quiet").is_empty());
        assert!(named("freehand", "freehand").is_empty());
        assert!(matches!(
            named("theme-quiet", "plugin/theme/quiet").as_slice(),
            [Problem::Name { expected, .. }] if expected == "uses-ink-theme-quiet"
        ));
        assert!(matches!(
            named("freehand-pen", "freehand").as_slice(),
            [Problem::Name { expected, .. }] if expected == "uses-ink-freehand"
        ));
    }

    #[test]
    fn a_crate_with_no_row_in_the_order_is_refused() {
        let found = problems_of(&Order, &workspace(vec![krate("uses-ink-new", "new", None, &[])], &[]));
        assert!(matches!(found.as_slice(), [Problem::Unordered { krate }] if krate == "uses-ink-new"));
    }

    #[test]
    fn a_dependency_the_order_forbids_is_refused() {
        let crates = vec![
            krate("uses-ink-model", "model", None, &[("uses-ink-config", None)]),
            krate("uses-ink-config", "config", None, &[("uses-ink-model", None)]),
        ];
        let found = problems_of(&Order, &workspace(crates, &[]));
        assert!(matches!(
            found.as_slice(),
            [Problem::Dependency { krate, on }] if krate == "uses-ink-model" && on == "uses-ink-config"
        ));
    }

    #[test]
    fn the_fixture_harness_is_for_dev_dependencies_only() {
        let normal = krate("uses-ink-engine", "engine", None, &[(Order::FIXTURE, None)]);
        assert!(matches!(problems_of(&Order, &workspace(vec![normal], &[])).as_slice(), [Problem::Fixture { .. }]));
        let dev = krate("uses-ink-engine", "engine", None, &[(Order::FIXTURE, Some(Kind::Dev))]);
        assert!(problems_of(&Order, &workspace(vec![dev], &[])).is_empty());
    }

    #[test]
    fn a_licence_is_the_one_its_kind_calls_for() {
        let check = |krate| problems_of(&Licence, &workspace(vec![krate], &[]));
        let model = |licence| krate("uses-ink-model", "model", licence, &[]);
        assert!(check(model(Some(Licence::PERMISSIVE))).is_empty());
        assert!(matches!(
            check(model(Some(Licence::SERVICE))).as_slice(),
            [Problem::Licence { expected, .. }] if *expected == Licence::PERMISSIVE
        ));
        let engine = krate("uses-ink-engine", "engine", None, &[]);
        assert!(matches!(
            check(engine).as_slice(),
            [Problem::Licence { found: None, expected, .. }] if *expected == Licence::SERVICE
        ));
    }

    #[test]
    fn a_permissive_crate_may_not_link_the_service() {
        let engine = || krate("uses-ink-engine", "engine", Some(Licence::SERVICE), &[]);
        let linked = krate("uses-ink-model", "model", Some(Licence::PERMISSIVE), &[("uses-ink-engine", None)]);
        let found = problems_of(&Licence, &workspace(vec![linked, engine()], &[]));
        assert!(matches!(
            found.as_slice(),
            [Problem::Copyleft { krate, on }] if krate == "uses-ink-model" && on == "uses-ink-engine"
        ));
        // a dev-dependency is not linked by whoever uses the crate
        let tested =
            krate("uses-ink-model", "model", Some(Licence::PERMISSIVE), &[("uses-ink-engine", Some(Kind::Dev))]);
        assert!(problems_of(&Licence, &workspace(vec![tested, engine()], &[])).is_empty());
    }

    #[test]
    fn a_crate_another_uses_needs_an_alias() {
        let crates = || {
            vec![
                krate("uses-ink-model", "model", None, &[]),
                krate("uses-ink-engine", "engine", None, &[("uses-ink-model", None)]),
            ]
        };
        let found = problems_of(&Alias, &workspace(crates(), &[]));
        assert!(matches!(found.as_slice(), [Problem::Alias { krate }] if krate == "uses-ink-model"));
        assert!(problems_of(&Alias, &workspace(crates(), &["uses-ink-model"])).is_empty());
    }
}
