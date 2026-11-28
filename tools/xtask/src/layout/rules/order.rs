//! The rule over dependency order: which crates of ours each crate may depend on
//! (docs/architecture.md section 4).

use super::names::{FARM, TASKS};
use crate::layout::model::{Kind, Workspace};
use crate::layout::{Problem, Problems, Rule};

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
        ("uses-ink-engine", &["uses-ink-model", "uses-ink-diag", "uses-ink-config", "uses-ink-logging"]),
        // Serving a deploy (decision 0051): the edge speaks the model alone; its adapters add the
        // runtime (axum) or a folder of deploys, never each other.
        ("uses-ink-edge", &["uses-ink-model", "uses-ink-logging"]),
        ("uses-ink-edge-axum", &["uses-ink-model", "uses-ink-edge", "uses-ink-edge-api", "uses-ink-logging"]),
        ("uses-ink-edge-folder", &["uses-ink-model", "uses-ink-edge", "uses-ink-logging"]),
        ("uses-ink-token", &[]),
        // The signed-request profile (decision 0068): the CLI signs and the server verifies with the
        // same code; it knows no crate of ours.
        ("uses-ink-sig", &[]),
        // Logging (decision 0065): a leaf every crate that instruments takes; it knows no crate of ours.
        ("uses-ink-logging", &[]),
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
                "uses-ink-queue-rows",
                "uses-ink-secret-local",
                "uses-ink-sig",
                "uses-ink-logging",
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
                "uses-ink-queue-rows",
                "uses-ink-secret-local",
                "uses-ink-logging",
            ],
        ),
        // The watch's Heads port over git's protocol (decision 0067): the watch's trait, git
        // through gix, the build port's revision and the model's locator.
        (
            "uses-ink-watch-git",
            &[
                "uses-ink-build",
                "uses-ink-https",
                "uses-ink-model",
                "uses-ink-secret",
                "uses-ink-logging",
                "uses-ink-watch",
            ],
        ),
        // How git reaches an https remote (decision 0072): gix, rustls and reqwest; no crate of ours.
        ("uses-ink-https", &[]),
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
                "uses-ink-queue-rows",
                "uses-ink-secret",
                "uses-ink-secret-local",
                "uses-ink-logging",
            ],
        ),
        // A format's native core speaks the model; its guest crate speaks the WIT (0041).
        ("uses-ink-format-markdown", &["uses-ink-model"]),
        // A dialect of markdown reads through the markdown format's comrak setup.
        ("uses-ink-format-obsidian", &["uses-ink-model", "uses-ink-format-markdown"]),
        ("uses-ink-host-image", &["uses-ink-model", "uses-ink-engine", "uses-ink-logging"]),
        ("uses-ink-host-native", &["uses-ink-https", "uses-ink-model", "uses-ink-engine", "uses-ink-logging"]),
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
                "uses-ink-sig",
                "uses-ink-edge-control",
                "uses-ink-clock",
                "uses-ink-control",
                "uses-ink-watch",
                "uses-ink-watch-git",
                "uses-ink-blob",
                "uses-ink-build",
                "uses-ink-build-process",
                "uses-ink-blob-dir",
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-queue-rows",
                "uses-ink-secret-local",
                "uses-ink-mail",
                "uses-ink-mail-smtp",
                "uses-ink-secret",
                "uses-ink-host-image",
                "uses-ink-host-native",
                "uses-ink-https",
                "uses-ink-format-markdown",
                "uses-ink-format-obsidian",
                "uses-ink-theme-quiet",
                "uses-ink-host-wasmtime",
                "uses-ink-logging",
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
                "uses-ink-logging",
            ],
        ),
        // The git source's native core speaks the model's `Git` trait; its tests run it over the
        // host's own fetching.
        ("uses-ink-source-git", &["uses-ink-model", "uses-ink-host-native", "uses-ink-logging"]),
        ("uses-ink-source-git-guest", &["uses-ink-model", "uses-ink-sdk", "uses-ink-source-git"]),
        ("uses-ink-sdk", &["freehand", "uses-ink-model"]),
        ("uses-ink-format-markdown-guest", &["uses-ink-model", "uses-ink-sdk", "uses-ink-format-markdown"]),
        ("uses-ink-format-obsidian-guest", &["uses-ink-model", "uses-ink-sdk", "uses-ink-format-obsidian"]),
        ("uses-ink-theme-quiet-guest", &["uses-ink-model", "uses-ink-engine", "uses-ink-sdk", "uses-ink-theme-quiet"]),
        ("uses-ink-host-wasmtime-hostile", &["uses-ink-sdk"]),
        // The control plane (decision 0045): ports are crates that know the model and nothing of
        // an adapter; an adapter knows its port and its conformance suite, which is dev-only.
        ("uses-ink-blob", &["uses-ink-model", "uses-ink-logging"]),
        ("uses-ink-blob-conformance", &["uses-ink-blob", "uses-ink-model"]),
        ("uses-ink-blob-dir", &["uses-ink-blob", "uses-ink-blob-conformance", "uses-ink-model", "uses-ink-logging"]),
        ("uses-ink-build", &["uses-ink-blob", "uses-ink-model", "uses-ink-logging"]),
        ("uses-ink-build-conformance", &["uses-ink-blob", "uses-ink-build", "uses-ink-model"]),
        (
            "uses-ink-build-process",
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
                "uses-ink-logging",
            ],
        ),
        // The row port holds only typed statement builders (0049); its adapters and suite are the
        // only crates that know a driver.
        ("uses-ink-db", &["uses-ink-logging"]),
        ("uses-ink-db-conformance", &["uses-ink-db"]),
        ("uses-ink-db-sqlite", &["uses-ink-db", "uses-ink-db-conformance", "uses-ink-logging"]),
        ("uses-ink-queue", &["uses-ink-model"]),
        ("uses-ink-queue-conformance", &["uses-ink-model", "uses-ink-queue"]),
        (
            "uses-ink-queue-rows",
            &[
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-model",
                "uses-ink-queue",
                "uses-ink-queue-conformance",
                "uses-ink-logging",
            ],
        ),
        // The mail port (decision 0066): typed messages and a trait, no adapter; the SMTP adapter
        // takes the port and the secret type that holds its password.
        ("uses-ink-mail", &[]),
        ("uses-ink-mail-smtp", &["uses-ink-mail", "uses-ink-secret"]),
        ("uses-ink-secret", &[]),
        ("uses-ink-secret-conformance", &["uses-ink-secret"]),
        ("uses-ink-secret-local", &["uses-ink-secret", "uses-ink-secret-conformance", "uses-ink-logging"]),
        // The floor the control plane's tables share (decision 0074): the database port, the model,
        // and the address and key types a stored column reads back as.
        ("uses-ink-control-rows", &["uses-ink-blob", "uses-ink-db", "uses-ink-mail", "uses-ink-model", "uses-ink-sig"]),
        // Signing in (decision 0066) over the row port alone: the rows' floor, the mail port for
        // links, the secret type a proof is held as, and the key type a credential names.
        (
            "uses-ink-control-signin",
            &["uses-ink-control-rows", "uses-ink-db", "uses-ink-mail", "uses-ink-secret", "uses-ink-sig"],
        ),
        // Keys for machines (decision 0068): the rows' floor, the sign-in's lifetimes and credentials, and
        // the signed-request profile's key types.
        ("uses-ink-control-keys", &["uses-ink-control-rows", "uses-ink-control-signin", "uses-ink-db", "uses-ink-sig"]),
        // The use cases know the ports; the adapters are what its tests run against.
        (
            "uses-ink-control",
            &[
                "uses-ink-blob",
                "uses-ink-blob-dir",
                "uses-ink-build",
                "uses-ink-control-keys",
                "uses-ink-control-rows",
                "uses-ink-control-signin",
                "uses-ink-db",
                "uses-ink-db-sqlite",
                "uses-ink-diag",
                "uses-ink-mail",
                "uses-ink-model",
                "uses-ink-queue",
                "uses-ink-queue-rows",
                "uses-ink-secret",
                "uses-ink-secret-local",
                "uses-ink-sig",
                "uses-ink-logging",
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

impl Order {
    /// The crates of ours that the crate named `name` may depend on; none when its row is missing.
    pub(crate) fn allowed(name: &str) -> Option<&'static [&'static str]> {
        Self::MAY_USE.iter().find(|(known, _)| *known == name).map(|&(_, allowed)| allowed)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::model::Crate;

    #[test]
    fn a_crate_with_no_row_in_the_order_is_refused() {
        let found =
            Problems::of(&Order, &Workspace::sample(vec![Crate::sample("uses-ink-new", "new", None, &[])], &[]));
        assert!(matches!(found.as_slice(), [Problem::Unordered { krate }] if krate == "uses-ink-new"));
    }

    #[test]
    fn a_dependency_the_order_forbids_is_refused() {
        let crates = vec![
            Crate::sample("uses-ink-model", "model", None, &[("uses-ink-config", None)]),
            Crate::sample("uses-ink-config", "config", None, &[("uses-ink-model", None)]),
        ];
        let found = Problems::of(&Order, &Workspace::sample(crates, &[]));
        assert!(matches!(
            found.as_slice(),
            [Problem::Dependency { krate, on }] if krate == "uses-ink-model" && on == "uses-ink-config"
        ));
    }

    #[test]
    fn the_fixture_harness_is_for_dev_dependencies_only() {
        let normal = Crate::sample("uses-ink-engine", "engine", None, &[(Order::FIXTURE, None)]);
        assert!(matches!(
            Problems::of(&Order, &Workspace::sample(vec![normal], &[])).as_slice(),
            [Problem::Fixture { .. }]
        ));
        let dev = Crate::sample("uses-ink-engine", "engine", None, &[(Order::FIXTURE, Some(Kind::Dev))]);
        assert!(Problems::of(&Order, &Workspace::sample(vec![dev], &[])).is_empty());
    }
}
