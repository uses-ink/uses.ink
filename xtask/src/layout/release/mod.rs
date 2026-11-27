//! The rule over release configuration: what dist builds, the workflow that runs it, and
//! the self-host image all name the same binary, and the image's command is one `ink` has
//! (docs/release.md).

mod dockerfile;
mod drift;
mod workflow;

use std::path::Path;

use heck::ToKebabCase as _;

use dockerfile::Dockerfile;
use workflow::Workflow;

use crate::layout::config::ReleasePaths;
use crate::layout::model::{Crate, Workspace};
use crate::layout::{Problem, Problems, Rule};

pub(crate) use drift::Drift;

/// The release files, read against the crates they name.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Release;

impl Release {
    fn text(root: &Path, name: &str) -> Result<String, Drift> {
        let path = root.join(name);
        std::fs::read_to_string(&path).map_err(|source| Drift::Unreadable { path: name.into(), source })
    }

    fn table(root: &Path, name: &str) -> Result<toml::Table, Drift> {
        let text = Self::text(root, name)?;
        toml::from_str(&text).map_err(|source| Drift::Unparsed { path: name.into(), source })
    }

    /// The crate dist releases and the binary it builds.
    fn binary(workspace: &Workspace) -> Result<(&Crate, String), Drift> {
        let mut marked = Vec::new();
        for krate in &workspace.crates {
            let manifest = Self::table(&workspace.root, &format!("{}/Cargo.toml", krate.place()))?;
            let dist = manifest.get("package").and_then(|p| p.get("metadata")).and_then(|m| m.get("dist"));
            if dist.and_then(|d| d.get("dist")).and_then(toml::Value::as_bool) == Some(true) {
                let bin = manifest.get("bin").and_then(toml::Value::as_array).and_then(|b| b.first());
                let name = bin.and_then(|b| b.get("name")).and_then(toml::Value::as_str);
                marked.push((krate, name.map(str::to_owned)));
            }
        }
        match marked.as_slice() {
            [(krate, Some(name))] => Ok((krate, name.clone())),
            [] | [(_, None)] => Err(Drift::NoBinary),
            many => Err(Drift::ManyBinaries { krates: many.iter().map(|(k, _)| k.name.clone()).collect() }),
        }
    }

    /// The commands of the binary, as a user types them: the variants of its `Command` enum.
    fn commands(krate: &Crate) -> Vec<String> {
        let items = krate.modules.iter().flat_map(|m| &m.syntax.items);
        items
            .filter_map(|item| match item {
                syn::Item::Enum(found) if found.ident == "Command" => Some(found),
                _ => None,
            })
            .flat_map(|found| found.variants.iter().map(|v| v.ident.to_string().to_kebab_case()))
            .collect()
    }

    fn config(root: &Path, paths: &ReleasePaths) -> Result<(String, usize), Drift> {
        let config = Self::table(root, &paths.config)?;
        let dist = config.get("dist").and_then(toml::Value::as_table);
        let version = dist.and_then(|d| d.get("cargo-dist-version")).and_then(toml::Value::as_str);
        let targets = dist.and_then(|d| d.get("targets")).and_then(toml::Value::as_array);
        Ok((version.unwrap_or_default().to_owned(), targets.map_or(0, Vec::len)))
    }

    /// The Dockerfile against the toolchain, the released crate and binary, and the commands.
    fn image(root: &Path, krate: &Crate, binary: &str, paths: &ReleasePaths) -> Result<Vec<Drift>, Drift> {
        let mut found = Vec::new();
        let dockerfile = Dockerfile::new(&Self::text(root, &paths.dockerfile)?);
        let channel = Self::table(root, &paths.toolchain)?
            .get("toolchain")
            .and_then(|t| t.get("channel"))
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let image = dockerfile.keyword("FROM").find_map(|rest| rest.strip_prefix("rust:"));
        let tag = image.map(|rest| rest.split(['-', '@', ' ']).next().unwrap_or(rest).to_owned());
        if tag.as_deref() != Some(channel.as_str()) {
            found.push(Drift::Toolchain { expected: channel, found: tag });
        }
        let runs = dockerfile.keyword("RUN").collect::<Vec<_>>().join("\n");
        let (built, embedded) = (runs.find("xtask component"), runs.find("cargo build"));
        if built.is_none() || embedded.is_none() || built > embedded {
            found.push(Drift::ImageComponents);
        }
        let package = dockerfile.after("--package");
        if package.as_deref() != Some(krate.name.as_str()) {
            found.push(Drift::Package { expected: krate.name.clone(), found: package });
        }
        let entry = dockerfile.exec("ENTRYPOINT").into_iter().next();
        if entry.as_deref() != Some(binary) {
            found.push(Drift::Entrypoint { expected: binary.to_owned(), found: entry });
        }
        found.extend(Self::command(&dockerfile, krate, paths));
        Ok(found)
    }

    /// The Dockerfile's `CMD` against the commands the binary knows and the ones still pending.
    fn command(dockerfile: &Dockerfile, krate: &Crate, paths: &ReleasePaths) -> Vec<Drift> {
        let mut found = Vec::new();
        let words = dockerfile.exec("CMD");
        let command = words.first().cloned();
        let known = Self::commands(krate);
        let on_all = words
            .windows(2)
            .any(|pair| matches!(pair, [flag, addr] if flag == "--addr" && addr.starts_with("0.0.0.0:")));
        if command.as_deref() == Some("serve") && !on_all {
            found.push(Drift::Listen { command: words.clone() });
        }
        match command.as_deref() {
            Some(name) if known.iter().any(|k| k == name) => {
                if let Some(pending) = paths.pending.iter().find(|p| **p == name) {
                    found.push(Drift::Pending { command: pending.clone() });
                }
            }
            Some(name) if paths.pending.iter().any(|p| p == name) => {}
            _ => found.push(Drift::Command { found: command, known }),
        }
        found
    }

    fn drift(workspace: &Workspace) -> Result<Vec<Drift>, Drift> {
        let root = workspace.root.as_path();
        let paths = &workspace.config.release;
        let mut found = Vec::new();
        let (krate, binary) = Self::binary(workspace)?;
        let (version, targets) = Self::config(root, paths)?;
        if targets == 0 {
            found.push(Drift::NoTargets { config: paths.config.clone() });
        }

        let workflow = Self::text(root, &paths.workflow)?;
        if !workflow.contains(&format!("cargo-dist/releases/download/v{version}/")) {
            found.push(Drift::Version {
                expected: version,
                path: paths.workflow.clone().into(),
                config: paths.config.clone(),
            });
        }
        if !workflow.contains("xtask component") {
            found.push(Drift::WorkflowComponents { path: paths.workflow.clone().into() });
        }
        for on in ["pull_request:", "pull_request_target:", "workflow_dispatch:", "schedule:", "branches:"] {
            if workflow.lines().any(|line| line.trim_start().starts_with(on)) {
                found.push(Drift::Trigger { path: paths.workflow.clone().into(), on: on.trim_end_matches(':') });
            }
        }

        if !workflow.contains("merge-base --is-ancestor") {
            found.push(Drift::Ancestry { path: paths.workflow.clone().into() });
        }
        for flow in Workflow::all(root, &paths.workflows)? {
            found.extend(flow.drift());
        }

        found.extend(Self::context(root, workspace)?);
        found.extend(Self::image(root, krate, &binary, paths)?);
        Ok(found)
    }

    /// The Docker context against the folders and files the build reads.
    fn context(root: &Path, workspace: &Workspace) -> Result<Vec<Drift>, Drift> {
        let paths = &workspace.config.release;
        let text = Self::text(root, &paths.ignore)?;
        let rules = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect::<Vec<_>>();
        let mut found = Vec::new();
        if rules.first() != Some(&"*") {
            found.push(Drift::OpenContext);
        }
        let allowed = rules.iter().filter_map(|rule| rule.strip_prefix('!')).map(|a| a.trim_end_matches('/'));
        let allowed = allowed.collect::<Vec<_>>();
        let places = workspace.crates.iter().map(Crate::place).chain(paths.root.iter().cloned());
        for place in places {
            if !allowed.iter().any(|a| place == *a || place.starts_with(&format!("{a}/"))) {
                found.push(Drift::Excluded { place });
            }
        }
        Ok(found)
    }
}

impl Rule for Release {
    const NAME: &'static str = "release";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        match Self::drift(workspace) {
            Ok(found) => found.into_iter().for_each(|drift| problems.push(Problem::Release(drift))),
            Err(drift) => problems.push(Problem::Release(drift)),
        }
    }
}
