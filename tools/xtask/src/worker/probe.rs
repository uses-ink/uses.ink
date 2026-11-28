//! What a host has for Worker runs, so a failed run can be told from a missing tool.

use super::case::Case;
use super::fault::Fault;
use super::shell::Shell;
use crate::report::{Line, Report};

/// The probe.
#[derive(Debug, Clone, Copy)]
pub(super) struct Probe<'a> {
    pub(super) shell: &'a Shell,
}

impl Probe<'_> {
    /// Each tool and the question put to it: its name, its words, and what it is asked for.
    const ASKS: &'static [(&'static str, &'static [&'static str])] = &[
        ("node", &["--version"]),
        ("npm", &["--version"]),
        ("npm", &["view", "wrangler", "version"]),
        ("npm", &["view", "wrangler", "dist-tags.latest", "dependencies.miniflare", "dependencies.workerd"]),
        ("rustup", &["target", "list", "--installed"]),
        ("gzip", &["--version"]),
        ("curl", &["-sSI", "-m", "20", "https://github.com/cloudflare/workerd/releases"]),
        ("curl", &["-sSI", "-m", "20", "https://registry.npmjs.org/wrangler"]),
    ];

    /// Asks each tool, and says what it answered, or that it did not.
    pub(super) fn run(&self, report: &mut Report) -> Result<(), Fault> {
        for (program, words) in Self::ASKS {
            let mut command = self.shell.command(program, self.shell.root());
            command.args(*words);
            let name = Shell::name(&command);
            let text = match self.shell.text(command) {
                Ok(text) => text,
                Err(fault) => format!("failed: {fault}"),
            };
            report.line(Line::Worker { case: Case::Probe, text: format!("$ {name}") });
            for line in text.lines().take(6) {
                report.line(Line::Worker { case: Case::Probe, text: format!("  {line}") });
            }
        }
        Ok(())
    }
}
