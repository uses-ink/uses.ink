//! `release plan` and `release build`: `dist` over `dist-workspace.toml`, which publishes nothing.

use std::fmt;
use std::path::Path;

use super::{Fault, Release};
use crate::build::component::Components;
use crate::check::Step;
use crate::error::Error;
use crate::process::Process;
use crate::report::{Line, Report};

/// A `dist` run.
#[derive(Debug, Clone, Copy)]
pub(super) enum Dist {
    /// `dist plan`.
    Plan,
    /// The components, then `dist build --artifacts=local` for this host's target.
    Build,
}

impl Dist {
    /// Where `dist build` writes, relative to the root.
    const OUT: &'static str = "target/distrib";

    /// The step of the task it is.
    const fn step(self) -> Release {
        match self {
            Self::Plan => Release::Plan,
            Self::Build => Release::Build,
        }
    }

    /// The Linux target of this machine; `dist build` builds the targets a host can.
    fn target() -> String {
        format!("{}-unknown-linux-gnu", std::env::consts::ARCH)
    }

    fn installed() -> bool {
        std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("dist").is_file()))
    }

    /// What it runs, after `dist`.
    fn args(self) -> Vec<String> {
        match self {
            Self::Plan => vec!["plan".to_owned()],
            Self::Build => ["build", "--artifacts=local", "--target"]
                .map(str::to_owned)
                .into_iter()
                .chain([Self::target()])
                .collect(),
        }
    }

    /// What the built binary answers to `--version`: it starts, and it is the one that was built.
    fn asked(root: &Path) -> Result<String, Fault> {
        let binary = root.join(Self::OUT).join(format!("uses-ink-cli-{}", Self::target())).join("ink");
        let mut command = std::process::Command::new(&binary);
        command.arg("--version");
        let line = format!("{} --version", binary.display());
        Process::from(command)
            .text(&[])
            .map(|text| text.trim().to_owned())
            .map_err(|source| Fault::Command { line, source })
    }

    /// The files `dist build` wrote, sorted.
    fn written(root: &Path) -> Result<Vec<String>, Fault> {
        let path = root.join(Self::OUT);
        let folder = |source| Fault::Folder { path: path.display().to_string(), source };
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&path).map_err(folder)? {
            names.push(entry.map_err(folder)?.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }
}

impl Step for Dist {
    fn run(&self, root: &Path, report: &mut Report) {
        let step = self.step();
        let fail = |fault| Error::Released { step, fault };
        if !Self::installed() {
            return report.fail(fail(Fault::NoDist));
        }
        let before = report.failures().len();
        if matches!(self, Self::Build) {
            Components.run(root, report);
            if report.failures().len() > before {
                return;
            }
        }
        let args = self.args();
        let mut command = std::process::Command::new("dist");
        command.args(&args).current_dir(root);
        let line = format!("dist {}", args.join(" "));
        if let Err(source) = Process::from(command).status() {
            return report.fail(fail(Fault::Command { line, source }));
        }
        match self {
            Self::Plan => {
                report.line(Line::Released { step, text: "the plan lists every artifact and no error".to_owned() });
            }
            Self::Build => {
                match Self::written(root).and_then(|names| Self::asked(root).map(|version| (names, version))) {
                    Ok((names, version)) => {
                        report.line(Line::Released {
                            step,
                            text: format!("built {}; the binary says `{version}`", names.join(", ")),
                        });
                    }
                    Err(fault) => report.fail(fail(fault)),
                }
            }
        }
    }
}

impl fmt::Display for Dist {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "dist {}", self.args().join(" "))
    }
}
