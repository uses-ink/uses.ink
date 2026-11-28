//! The rule data in `.config/layout.toml`: the lists of crates and paths the layout rules check
//! against, read once into a typed [`Config`]. A row that is not what the rules expect is refused
//! with the file and the line.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::docs::Number;
use crate::error::Error;

/// Every list the rules read, by the rule that reads it.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    pub(crate) enforced: Enforced,
    #[serde(rename = "unsafe")]
    pub(crate) risky: Allowed,
    pub(crate) free: Exceptions,
    pub(crate) sql: SqlPaths,
    pub(crate) release: ReleasePaths,
    pub(crate) layers: Layers,
    pub(crate) fanout: Fanout,
    pub(crate) decisions: Numbering,
    pub(crate) docs: Prose,
}

/// The crates a gradual rule fails.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Enforced {
    pub(crate) crates: Vec<String>,
}

/// The files that may hold unsafe code.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Allowed {
    pub(crate) allowed: Vec<String>,
}

/// The free functions allowed outside the entry points.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Exceptions {
    pub(crate) allowed: Vec<Exception>,
}

/// A free function allowed, and why.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Exception {
    pub(crate) path: String,
    pub(crate) name: String,
    /// Why no type can hold it; read by people, not by the rule.
    #[expect(dead_code, reason = "the reason is written for the reader of the list")]
    reason: String,
}

/// Where the SQL rule looks away.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SqlPaths {
    pub(crate) own: Vec<String>,
    pub(crate) foreign: Vec<String>,
}

/// The files the release rule reads, relative to the root.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleasePaths {
    /// The dist config.
    pub(crate) config: String,
    pub(crate) dockerfile: String,
    pub(crate) ignore: String,
    /// Commands the image runs that the binary does not have yet. The rule fails once a command
    /// exists and is still listed, so this list only shrinks.
    pub(crate) pending: Vec<String>,
    /// Files outside every crate that the image build reads.
    pub(crate) root: Vec<String>,
    pub(crate) toolchain: String,
    pub(crate) workflow: String,
    /// The folder of every workflow.
    pub(crate) workflows: String,
}

/// The layers a crate path may sit under.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Layers {
    pub(crate) order: Vec<String>,
    /// Layers of crates that stand alone: any crate may depend on them, and they depend on
    /// nothing outside their own layer.
    #[serde(default)]
    pub(crate) open: Vec<String>,
}

impl Layers {
    /// Whether a crate in layer `from` may depend on one in layer `to`: its own layer, an open
    /// one, or an earlier one of the order. An open layer depends on itself alone.
    pub(crate) fn allows(&self, from: &str, to: &str) -> bool {
        let open = |layer: &str| self.open.iter().any(|known| known == layer);
        if from == to || (open(to) && !open(from)) {
            return true;
        }
        let rank = |layer: &str| self.order.iter().position(|known| known == layer);
        !open(from) && rank(to) < rank(from)
    }
}

/// The fan-out rule: how many entries a directory holds, and what a break of it does.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Fanout {
    pub(crate) limit: usize,
    pub(crate) mode: Strictness,
    /// Path parts whose directories are not checked.
    pub(crate) exempt: Vec<String>,
    #[serde(default)]
    pub(crate) waive: Vec<Waiver>,
}

/// What a directory over the limit does.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Strictness {
    /// A note.
    #[default]
    Report,
    /// A failure.
    Enforce,
}

/// A directory allowed over the limit, and why.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Waiver {
    pub(crate) path: String,
    /// Why it is allowed; read by people, not by the rule.
    #[expect(dead_code, reason = "the reason is written for the reader of the list")]
    reason: String,
}

/// The numbers of the decisions: how far a bare `(NNNN)` is a citation, which numbers are
/// reserved, and which are gaps.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Numbering {
    /// The highest number a bare `(NNNN)` is a citation of; above it the digits are a file
    /// mode, a year or a count. Raise it before the numbers reach it.
    pub(crate) ceiling: u16,
    /// Where the reserved numbers went, relative to `docs/decisions/`.
    pub(crate) home: String,
    #[serde(default)]
    pub(crate) reserved: Vec<Reserved>,
    #[serde(default)]
    pub(crate) gap: Vec<Gap>,
}

/// A number held back for something that is no decision: a row of the index, and a heading at
/// its home.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reserved {
    pub(crate) number: Number,
    /// The title the index shows.
    pub(crate) title: String,
}

/// Numbers, first to last, that no decision holds, and why.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Gap {
    pub(crate) first: Number,
    pub(crate) last: Number,
    /// Why nothing holds them; read by people, not by the check.
    #[expect(dead_code, reason = "the reason is written for the reader of the list")]
    reason: String,
}

/// What the docs checks read besides the docs.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Prose {
    /// Words (case does not matter) that mark a block of prose as about what is not built, so
    /// the paths it names need not exist.
    pub(crate) planned: Vec<String>,
    /// Docs whose paths need not exist, each with the reason.
    #[serde(default)]
    pub(crate) future: Vec<Future>,
    /// Files whose citations are not checked, each with the reason.
    #[serde(default)]
    pub(crate) quoted: Vec<Quoted>,
}

/// A doc or a folder of docs that names paths that are gone or not yet there.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Future {
    pub(crate) path: String,
    /// Why; read by people, not by the check.
    #[expect(dead_code, reason = "the reason is written for the reader of the list")]
    reason: String,
}

/// A file or folder that holds citations as examples, not as references.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Quoted {
    pub(crate) path: String,
    /// Why; read by people, not by the check.
    #[expect(dead_code, reason = "the reason is written for the reader of the list")]
    reason: String,
}

#[cfg(test)]
impl Reserved {
    /// The reserved number `number` titled `title`.
    pub(crate) fn sample(number: u16, title: &str) -> Self {
        Self { number: Number::sample(number), title: title.to_owned() }
    }
}

#[cfg(test)]
impl Gap {
    /// The gap from `first` to `last`.
    pub(crate) fn sample(first: u16, last: u16) -> Self {
        Self { first: Number::sample(first), last: Number::sample(last), reason: String::new() }
    }
}

#[cfg(test)]
impl Future {
    /// A doc at `path` that names paths not yet there.
    pub(crate) fn sample(path: &str) -> Self {
        Self { path: path.to_owned(), reason: String::new() }
    }
}

#[cfg(test)]
impl Quoted {
    /// A file at `path` that quotes citations.
    pub(crate) fn sample(path: &str) -> Self {
        Self { path: path.to_owned(), reason: String::new() }
    }
}

impl Config {
    /// Where the file is, relative to the root.
    pub(crate) const PATH: &'static str = ".config/layout.toml";

    /// Reads and checks the file under `root`.
    pub(crate) fn read(root: &Path) -> Result<Self, Error> {
        let path = root.join(Self::PATH);
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Read { path, source })?;
        Self::parse(&text)
    }

    /// Checks `text` as the file.
    pub(crate) fn parse(text: &str) -> Result<Self, Error> {
        toml::from_str(text).map_err(|source| {
            let line = source.span().map(|span| text[..span.start].matches('\n').count() + 1);
            Error::Config { path: PathBuf::from(Self::PATH), line, source: Box::new(source) }
        })
    }

    /// Whether `krate` is one a gradual rule fails.
    pub(crate) fn enforces(&self, krate: &str) -> bool {
        self.enforced.crates.iter().any(|name| name == krate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every crate the rule list held in code before it became data, sorted.
    const ENFORCED: [&str; 70] = [
        "farm",
        "freehand",
        "freehand-web",
        "uses-ink-authority-dns",
        "uses-ink-authority-email",
        "uses-ink-authority-forge",
        "uses-ink-authority-forgejo",
        "uses-ink-authority-github",
        "uses-ink-authority-gitlab",
        "uses-ink-blob",
        "uses-ink-blob-conformance",
        "uses-ink-blob-dir",
        "uses-ink-blob-r2",
        "uses-ink-build",
        "uses-ink-build-conformance",
        "uses-ink-build-process",
        "uses-ink-cli",
        "uses-ink-clock",
        "uses-ink-config",
        "uses-ink-control",
        "uses-ink-control-keys",
        "uses-ink-control-rows",
        "uses-ink-control-signin",
        "uses-ink-db",
        "uses-ink-db-conformance",
        "uses-ink-db-d1",
        "uses-ink-db-sqlite",
        "uses-ink-diag",
        "uses-ink-edge",
        "uses-ink-edge-api",
        "uses-ink-edge-axum",
        "uses-ink-edge-control",
        "uses-ink-edge-folder",
        "uses-ink-engine",
        "uses-ink-fixture",
        "uses-ink-format-markdown",
        "uses-ink-format-markdown-guest",
        "uses-ink-format-obsidian",
        "uses-ink-format-obsidian-guest",
        "uses-ink-host-image",
        "uses-ink-host-native",
        "uses-ink-host-wasmtime",
        "uses-ink-host-wasmtime-hostile",
        "uses-ink-https",
        "uses-ink-logging",
        "uses-ink-mail",
        "uses-ink-mail-cf",
        "uses-ink-mail-conformance",
        "uses-ink-mail-smtp",
        "uses-ink-model",
        "uses-ink-queue",
        "uses-ink-queue-cf",
        "uses-ink-queue-conformance",
        "uses-ink-queue-rows",
        "uses-ink-sdk",
        "uses-ink-secret",
        "uses-ink-secret-conformance",
        "uses-ink-secret-local",
        "uses-ink-secret-seal",
        "uses-ink-secret-worker",
        "uses-ink-sig",
        "uses-ink-source-git",
        "uses-ink-source-git-guest",
        "uses-ink-theme-quiet",
        "uses-ink-theme-quiet-guest",
        "uses-ink-token",
        "uses-ink-watch",
        "uses-ink-watch-git",
        "uses-ink-web",
        "xtask",
    ];

    fn real() -> Config {
        Config::read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
    }

    fn sorted(list: &[String]) -> Vec<&str> {
        let mut list: Vec<&str> = list.iter().map(String::as_str).collect();
        list.sort_unstable();
        list
    }

    #[test]
    fn the_real_file_lists_the_crates_the_code_enforced() {
        assert_eq!(sorted(&real().enforced.crates), ENFORCED);
    }

    #[test]
    fn the_real_file_holds_the_paths_the_code_named() {
        let config = real();
        assert_eq!(
            sorted(&config.risky.allowed),
            [
                "crates/runtime/host/wasmtime/hostile/src/bind.rs",
                "crates/runtime/host/wasmtime/src/cache.rs",
                "crates/runtime/plugin/format/markdown/guest/src/bind.rs",
                "crates/runtime/plugin/format/obsidian/guest/src/bind.rs",
                "crates/runtime/plugin/source/git/guest/src/bind.rs",
                "crates/runtime/plugin/theme/quiet/guest/src/bind.rs",
                "crates/standalone/freehand-web/src/lib.rs",
            ]
        );
        assert_eq!(config.free.allowed.len(), 0);
        assert_eq!(config.sql.own, ["tools/xtask/src/layout/rules/sql.rs"]);
        assert_eq!(config.sql.foreign, ["spikes", "target"]);
        let release = &config.release;
        assert_eq!(release.config, "dist-workspace.toml");
        assert_eq!(release.workflow, ".github/workflows/release.yml");
        assert_eq!(release.workflows, ".github/workflows");
        assert_eq!(release.ignore, ".dockerignore");
        assert_eq!(release.dockerfile, "Dockerfile");
        assert_eq!(release.toolchain, "rust-toolchain.toml");
        assert_eq!(sorted(&release.root), ["Cargo.lock", "Cargo.toml", "rust-toolchain.toml"]);
        assert_eq!(release.pending.len(), 0);
    }

    #[test]
    fn the_real_file_sets_the_layers_and_the_fanout() {
        let config = real();
        assert_eq!(config.layers.order, ["core", "port", "server", "runtime", "surface", "standalone"]);
        assert_eq!(config.layers.open, ["standalone"]);
        assert_eq!(config.fanout.limit, 8);
        assert_eq!(config.fanout.mode, Strictness::Enforce);
    }

    #[test]
    fn the_real_file_sets_the_decisions_and_the_docs_checks() {
        let config = real();
        assert_eq!(config.decisions.ceiling, 199);
        assert_eq!(config.decisions.home, "../history/review-fixes.md");
        let reserved: Vec<u16> = config.decisions.reserved.iter().map(|r| r.number.get()).collect();
        assert_eq!(reserved, [58, 60]);
        assert_eq!(config.decisions.gap.len(), 10);
        assert!(config.docs.planned.iter().any(|word| word == "planned"));
        assert!(config.docs.future.iter().any(|entry| entry.path == "docs/layout.md"), "the layout plan is waived");
        assert_eq!(config.docs.quoted.len(), 1);
    }

    #[test]
    fn a_number_that_is_not_four_digits_is_refused_with_its_line() {
        let text =
            "[decisions]\nceiling = 99\nhome = \"x\"\n\n[[decisions.reserved]]\nnumber = \"58\"\ntitle = \"t\"\n";
        assert!(matches!(Config::parse(text), Err(Error::Config { line: Some(_), .. })));
    }

    #[test]
    fn a_wrong_row_is_refused_with_its_line() {
        let text = "[enforced]\ncrates = []\n\n[unsafe]\nallowed = 3\n";
        let Err(Error::Config { line, path, .. }) = Config::parse(text) else { panic!("accepted") };
        assert_eq!(line, Some(5));
        assert_eq!(path, Path::new(Config::PATH));
    }

    #[test]
    fn an_unknown_key_is_refused() {
        assert!(matches!(Config::parse("[enforced]\ncrate = []\n"), Err(Error::Config { line: Some(2), .. })));
    }
}
