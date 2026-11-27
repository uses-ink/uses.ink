//! Compiler diagnostics read from cargo output.

use std::fmt;

use serde::Serialize;

use super::{DIAGNOSTICS, LEAD};

/// What the compiler said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Diagnostics {
    pub(crate) errors: u32,
    pub(crate) warnings: u32,
    /// The first [`DIAGNOSTICS`], errors first, in short form.
    pub(crate) first: Vec<String>,
    /// The first error as the compiler printed it, at most [`LEAD`] lines: head, place, code.
    pub(crate) lead: Vec<String>,
}

/// One diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Diagnostic {
    error: bool,
    code: Option<String>,
    /// The lint that raised it, as the compiler names it in a note: `clippy::too-many-lines`.
    lint: Option<String>,
    /// What the compiler printed under the head, up to the blank line.
    body: Vec<String>,
    at: String,
    message: String,
}

impl Diagnostic {
    /// Reads `error[E0425]: cannot find value` / `warning: unused variable`.
    fn head(line: &str) -> Option<(bool, Option<String>, &str)> {
        let (error, rest) = match (line.strip_prefix("error"), line.strip_prefix("warning")) {
            (Some(rest), _) => (true, rest),
            (_, Some(rest)) => (false, rest),
            _ => return None,
        };
        let (code, message) = match rest.strip_prefix('[') {
            Some(inner) => {
                let (code, tail) = inner.split_once("]: ")?;
                (Some(code.to_owned()), tail)
            }
            None => (None, rest.strip_prefix(": ")?),
        };
        let summary = [
            "could not compile",
            "aborting due to",
            "test run failed",
            "build failed",
            "check:",
            "layout problems",
            "stale generated",
        ];
        if summary.iter().any(|start| message.starts_with(start)) || message.contains(" generated ") {
            return None;
        }
        Some((error, code, message))
    }

    /// Reads `--> src/lib.rs:10:23`.
    fn place(line: &str) -> Option<&str> {
        line.trim_start().strip_prefix("--> ")
    }
}

impl Diagnostic {
    /// Lines that only repeat what the head already says or advertise how to silence it.
    const NOISE: [&'static str; 3] = ["implied by `-D", "to override `-D", "for further information visit"];

    /// The lint a note under the head names: `` `-D clippy::x` implied by ``, `` `#[warn(x)]` ``
    /// or `` `cargo::x` is set to ``.
    fn lint(body: &[String]) -> Option<String> {
        body.iter().find_map(|line| {
            let note = line.trim_start().strip_prefix("= note: `")?;
            let name = if let Some(flag) = note.strip_prefix("-D ") {
                flag.split_once('`')?.0
            } else if let Some(attr) = note.strip_prefix("#[") {
                attr.split_once('(')?.1.split_once(')')?.0
            } else if note.contains("` is set to `") {
                note.split_once('`')?.0
            } else {
                return None;
            };
            Some(name.to_owned())
        })
    }

    /// The head, then the code under it: at most [`LEAD`] lines, no blank ones, no noise.
    fn block(&self) -> Vec<String> {
        let code = self
            .body
            .iter()
            .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with("--> "))
            .filter(|line| !Self::NOISE.iter().any(|noise| line.contains(noise)))
            .take(LEAD - 1)
            .map(|line| line.trim_end().to_owned());
        std::iter::once(self.to_string()).chain(code).collect()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = if self.error { "error" } else { "warning" };
        match self.code.as_ref().or(self.lint.as_ref()) {
            Some(code) => write!(out, "{level}[{code}] {}: {}", self.at, self.message),
            None => write!(out, "{level} {}: {}", self.at, self.message),
        }
    }
}

impl Diagnostics {
    pub(super) fn of(log: &str) -> Self {
        let mut found: Vec<Diagnostic> = Vec::new();
        let lines: Vec<&str> = log.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            let Some((error, code, message)) = Diagnostic::head(line) else { continue };
            let place = lines.iter().skip(at + 1).take(3).find_map(|next| Diagnostic::place(next));
            let linking = message.starts_with("linking with") || message.starts_with("linker");
            let Some(place) = place.or(linking.then_some("link")) else { continue };
            let body: Vec<String> = lines
                .iter()
                .skip(at + 1)
                .take_while(|next| !next.trim().is_empty() && Diagnostic::head(next).is_none())
                .map(|next| (*next).to_owned())
                .collect();
            let lint = Diagnostic::lint(&body);
            let diagnostic = Diagnostic { error, code, lint, body, at: place.to_owned(), message: message.to_owned() };
            // the same diagnostic printed again (a rerun, another target) counts once
            if !found.iter().any(|known| known.to_string() == diagnostic.to_string()) {
                found.push(diagnostic);
            }
        }
        let errors = found.iter().filter(|d| d.error).count();
        found.sort_by_key(|d| !d.error);
        let lead = found.first().filter(|d| d.error).map_or_else(Vec::new, Diagnostic::block);
        Self {
            lead,
            errors: u32::try_from(errors).unwrap_or(u32::MAX),
            warnings: u32::try_from(found.len() - errors).unwrap_or(u32::MAX),
            first: found.iter().take(DIAGNOSTICS).map(ToString::to_string).collect(),
        }
    }
}
