//! The rule data in `.config/layout.toml`: the lists of crates and paths the layout rules check
//! against, read once into a typed [`Config`]. A row that is not what the rules expect is refused
//! with the file and the line.

use std::path::{Path, PathBuf};

use serde::Deserialize;

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
    const ENFORCED: [&str; 54] = [
        "farm",
        "freehand",
        "freehand-web",
        "uses-ink-blob",
        "uses-ink-blob-conformance",
        "uses-ink-blob-dir",
        "uses-ink-build",
        "uses-ink-build-conformance",
        "uses-ink-build-local",
        "uses-ink-cli",
        "uses-ink-clock",
        "uses-ink-config",
        "uses-ink-control",
        "uses-ink-db",
        "uses-ink-db-conformance",
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
        "uses-ink-mail",
        "uses-ink-mail-smtp",
        "uses-ink-model",
        "uses-ink-queue",
        "uses-ink-queue-conformance",
        "uses-ink-queue-local",
        "uses-ink-sdk",
        "uses-ink-secret",
        "uses-ink-secret-conformance",
        "uses-ink-secret-local",
        "uses-ink-sig",
        "uses-ink-source-git",
        "uses-ink-source-git-guest",
        "uses-ink-theme-quiet",
        "uses-ink-theme-quiet-guest",
        "uses-ink-token",
        "uses-ink-trace",
        "uses-ink-watch",
        "uses-ink-watch-git",
        "uses-ink-web",
        "xtask",
    ];

    fn real() -> Config {
        Config::read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap()
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
                "freehand-web/src/lib.rs",
                "host/wasmtime/hostile/src/bind.rs",
                "host/wasmtime/src/cache.rs",
                "plugin/format/markdown/guest/src/bind.rs",
                "plugin/format/obsidian/guest/src/bind.rs",
                "plugin/source/git/guest/src/bind.rs",
                "plugin/theme/quiet/guest/src/bind.rs",
            ]
        );
        assert_eq!(config.free.allowed.len(), 0);
        assert_eq!(config.sql.own, ["xtask/src/layout/rules/sql.rs"]);
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
        assert_eq!(config.fanout.limit, 8);
        assert_eq!(config.fanout.mode, Strictness::Report);
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
