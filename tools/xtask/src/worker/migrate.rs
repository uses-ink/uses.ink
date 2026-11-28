//! H1b: the migrations of the conformance suite, written as wrangler files by the port's renderer,
//! applied to a local D1 with `wrangler d1 migrations apply --local`, and the schema and ledger
//! that result compared with what the SQLite adapter makes of the same migrations (decision 0098).

use std::path::PathBuf;

use serde_json::Value;

use super::case::Case;
use super::fault::Fault;
use super::node::Packages;
use super::node::Shell;
use crate::report::{Line, Report};

/// One run.
#[derive(Debug, Clone, Copy)]
pub(super) struct Migrated<'a> {
    pub(super) shell: &'a Shell,
}

impl Migrated<'_> {
    /// The folder this run works in, under `target/`.
    fn work(self) -> PathBuf {
        self.shell.root().join(Shell::HOME).join("migrate")
    }

    /// `wrangler d1 <verbs> DB --local` on the local database of this run.
    fn wrangler(self, verbs: &[&str]) -> std::process::Command {
        let mut command = self.shell.command("wrangler", self.shell.root());
        command.arg("d1").args(verbs);
        command.args(["DB", "--local", "--config", "spikes/hosted/workers/migrate/wrangler.toml", "--persist-to"]);
        command.arg(self.work().join("state"));
        command
    }

    /// The rows wrangler answers to the query in `file`, as the lines `expected.tsv` has.
    fn found(self, file: &str, kind: &str, columns: &[&str]) -> Result<Vec<String>, Fault> {
        let mut command = self.wrangler(&["execute"]);
        command.arg("--file").arg(self.work().join("out").join(file)).arg("--json");
        let text = Shell::text(command)?;
        let start = text.find('[').unwrap_or(0);
        let json: Value = serde_json::from_str(text.get(start..).unwrap_or_default())
            .map_err(|source| Fault::Json { what: file.to_owned(), source })?;
        let mut lines = Vec::new();
        for result in json.as_array().into_iter().flatten() {
            for row in result.get("results").and_then(Value::as_array).into_iter().flatten() {
                let cells: Vec<&str> =
                    columns.iter().map(|column| row.get(column).and_then(Value::as_str).unwrap_or("")).collect();
                lines.push(format!("{kind}\t{}", cells.join("\t")));
            }
        }
        Ok(lines)
    }

    /// Writes the files, applies them, and compares.
    pub(super) fn run(self, report: &mut Report) -> Result<(), Fault> {
        let mut say = |text: String| report.line(Line::Worker { case: Case::Migrate, text });
        Packages { shell: self.shell }.install()?;
        let work = self.work();
        match std::fs::remove_dir_all(&work) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(Fault::File { path: work, source }),
        }
        let mut write = self.shell.command(env!("CARGO"), self.shell.root());
        write
            .args(["run", "--quiet", "-p", "uses-ink-db-sqlite", "--features", "scripts", "--bin", "scripts", "--"])
            .arg(work.join("out"));
        Shell::run(write)?;
        let expected = String::from_utf8_lossy(&Shell::read(&work.join("out/expected.tsv"))?).into_owned();
        let apply = self.wrangler(&["migrations", "apply"]);
        say(format!("applying with {}", Shell::name(&apply)));
        Shell::run(apply)?;
        let mut found = self.found("schema.sql", "schema", &["type", "name", "tbl_name", "sql"])?;
        found.extend(self.found("ledger.sql", "ledger", &["name"])?);
        let expected: Vec<String> = expected.lines().map(str::to_owned).collect();
        if found != expected {
            return Err(Fault::Differs { expected: expected.join("\n"), found: found.join("\n") });
        }
        let objects = expected.iter().filter(|line| line.starts_with("schema")).count();
        let applied = expected.len() - objects;
        say(format!("ok same schema as SQLite: {objects} objects, {applied} ledger rows, byte for byte"));
        for line in &expected {
            say(format!("  {line}"));
        }
        Ok(())
    }
}
