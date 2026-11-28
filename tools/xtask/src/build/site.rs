//! `ink build --check` on every site fixture.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::check::{Cargo, Command, Step, Target};
use crate::error::Error;
use crate::process::Process;
use crate::report::{Line, Report};

/// What `ink build --check --json` says: the deploy each way, or none when the site did not
/// render (a fixture may fail on purpose, the same way both times).
#[derive(Debug, serde::Deserialize)]
struct Compared {
    native: Option<String>,
    component: Option<String>,
}

/// `ink build --check` on every CLI site fixture: each builds the same deploy natively and
/// through the components `ink` carries, so this is also the test that it carries them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sites;

impl Sites {
    /// The site fixtures, relative to the root.
    const DIR: &'static str = "crates/surface/cli/fixture/site";

    /// The cache their builds share, relative to the root.
    const CACHE: &'static str = "target/ink-cache";

    const BUILD: Cargo<[&'static str; 1]> =
        Cargo { command: Command::Build, scope: ["uses-ink-cli"], flags: &[], target: Target::Host };

    /// Every fixture folder, sorted.
    fn fixtures(root: &Path) -> Result<Vec<PathBuf>, Error> {
        let dir = root.join(Self::DIR);
        let entries = std::fs::read_dir(&dir).map_err(|source| Error::Read { path: dir.clone(), source })?;
        let mut fixtures = Vec::new();
        for entry in entries {
            let path = entry.map_err(|source| Error::Read { path: dir.clone(), source })?.path();
            if path.is_dir() {
                fixtures.push(path);
            }
        }
        fixtures.sort();
        Ok(fixtures)
    }
}

impl Step for Sites {
    fn run(&self, root: &Path, report: &mut Report) {
        let before = report.failures().len();
        Self::BUILD.run(root, report);
        if report.failures().len() > before {
            return;
        }
        let fixtures = match Self::fixtures(root) {
            Ok(fixtures) => fixtures,
            Err(error) => return report.fail(error),
        };
        let ink = root.join("target/debug/ink");
        for fixture in fixtures {
            let mut command = std::process::Command::new(&ink);
            command
                .args(["--json", "build", "--check"])
                .arg(&fixture)
                // kept under target/, never in the fixture
                .arg("--cache")
                .arg(root.join(Self::CACHE));
            let path = fixture.strip_prefix(root).unwrap_or(&fixture).to_owned();
            match Process::from(command).answer::<Compared>() {
                Ok(compared) if compared.native == compared.component => report.line(Line::Matched(path)),
                Ok(compared) => {
                    report.fail(Error::Mismatch { path, native: compared.native, component: compared.component });
                }
                Err(source) => report.fail(Error::Site { path, source }),
            }
        }
    }
}

impl fmt::Display for Sites {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}, then `ink build --check` on every folder of {}", Self::BUILD, Self::DIR)
    }
}
