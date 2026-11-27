//! The rules over files: banned names anywhere in the tree, unsafe code only where a decision
//! says.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use proc_macro2::{LexError, TokenStream, TokenTree};

use crate::layout::config::Strictness;
use crate::layout::model::Workspace;
use crate::layout::{Problem, Problems, Rule};

/// No file or directory named for a heap of unrelated code.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Banned;

impl Banned {
    /// Names that collect unrelated code.
    const NAMES: &'static [&'static str] = &[
        "util", "utils", "common", "core", "helpers", "misc", "base", "shared", "manager", "handler", "service", "impl",
    ];
}

impl Rule for Banned {
    const NAME: &'static str = "banned";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for path in &workspace.entries {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy()) else { continue };
            let stem = name.split('.').next().unwrap_or(&name);
            if Self::NAMES.contains(&stem) {
                problems.push(Problem::Banned { path: path.clone() });
            }
        }
    }
}

/// Unsafe code only in the files `unsafe.allowed` lists in `.config/layout.toml`. A file holds it when the keyword,
/// or the lint that guards it, is a token of its code: comments and literals do not count.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Unsafe;

impl Unsafe {
    /// The keyword, spelled in two parts so this file does not hold it.
    pub(crate) const KEYWORD: &'static str = concat!("un", "safe");

    /// The lint that guards the keyword, which only a file allowed unsafe code may name.
    const LINT: &'static str = concat!("un", "safe_code");

    /// Whether `text` holds the keyword or names its lint as code.
    fn holds(text: &str) -> Result<bool, LexError> {
        text.parse::<TokenStream>().map(Self::within)
    }

    fn within(tokens: TokenStream) -> bool {
        tokens.into_iter().any(|token| match token {
            TokenTree::Ident(ident) => ident == Self::KEYWORD || ident == Self::LINT,
            TokenTree::Group(group) => Self::within(group.stream()),
            TokenTree::Punct(_) | TokenTree::Literal(_) => false,
        })
    }
}

impl Rule for Unsafe {
    const NAME: &'static str = "unsafe";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for file in workspace.crates.iter().flat_map(|c| &c.files) {
            let allowed = workspace.config.risky.allowed.iter().any(|a| file.path == Path::new(a));
            match Self::holds(&file.text) {
                Ok(holds) if holds && !allowed => problems.push(Problem::Unsafe { path: file.path.clone() }),
                Ok(_) => {}
                Err(source) => problems.push(Problem::Lexed { path: file.path.clone(), source }),
            }
        }
    }
}

/// A directory holds at most `fanout.limit` entries, so a reader sees about eight things at a time.
/// The root counts its directories only. A directory with a part named in `fanout.exempt` is not
/// checked, and a waiver in `.config/layout.toml` allows one by path. With `mode = "report"` a
/// directory over the limit is a note, with `"enforce"` a failure.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fanout;

impl Fanout {
    /// How many entries each directory of `entries` holds; the root's count is its directories.
    fn counts(entries: &[PathBuf]) -> BTreeMap<PathBuf, usize> {
        let parents: Vec<&Path> = entries.iter().filter_map(|entry| entry.parent()).collect();
        let mut counts = BTreeMap::new();
        for entry in entries {
            let dir = entry.parent().unwrap_or(Path::new("")).to_path_buf();
            let counted = !dir.as_os_str().is_empty() || parents.contains(&entry.as_path());
            if counted {
                *counts.entry(dir).or_insert(0) += 1;
            }
        }
        counts
    }
}

impl Rule for Fanout {
    const NAME: &'static str = "fanout";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let fanout = &workspace.config.fanout;
        problems.limit = fanout.limit;
        let exempt = |dir: &Path| {
            dir.components().any(|part| fanout.exempt.iter().any(|name| part.as_os_str() == name.as_str()))
        };
        for (dir, entries) in Self::counts(&workspace.entries) {
            let waived = fanout.waive.iter().any(|waiver| dir == Path::new(&waiver.path));
            if entries <= fanout.limit || waived || exempt(&dir) {
                continue;
            }
            match fanout.mode {
                Strictness::Report => problems.crowded.push((dir, entries)),
                Strictness::Enforce => problems.push(Problem::Crowded { dir, entries, limit: fanout.limit }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn a_banned_name_is_refused_as_a_file_or_a_directory() {
        let entries = ["src/utils.rs", "src/common", "src/lib.rs", "src/util.rs"].map(PathBuf::from).to_vec();
        let mut workspace = Workspace::sample(vec![], &[]);
        workspace.entries = entries;
        let mut problems = Problems::default();
        Banned.check(&workspace, &mut problems);
        let found: Vec<&PathBuf> = problems
            .found
            .iter()
            .filter_map(|problem| match problem {
                Problem::Banned { path } => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(
            found,
            [&PathBuf::from("src/utils.rs"), &PathBuf::from("src/common"), &PathBuf::from("src/util.rs")]
        );
        assert_eq!(problems.found.len(), 3);
    }

    fn holds(code: &str) -> bool {
        Unsafe::holds(code).unwrap()
    }

    #[test]
    fn unsafe_in_every_spelling() {
        let k = Unsafe::KEYWORD;
        for code in [
            format!("{k} {{ x }}"),
            format!("{k}{{*p}}"),
            format!("{k}\n{{ }}"),
            format!("pub {k} fn f() {{}}"),
            format!("{k} impl Send for X {{}}"),
            format!("#[{k}(no_mangle)]"),
            format!("#![expect({k}_code)]"),
            format!("let s = \"a\"; {k} {{ x }}"),
            format!("/* a */ {k} {{ x }}"),
            format!("let c = '\"'; {k} {{ x }}"),
            format!("fn f<'a>(x: &'a u8) {{ {k} {{ x }} }}"),
            format!("let s = r#\"a\"#; {k} {{ x }}"),
            format!("let s = \"\\\"\"; {k} {{ x }}"),
        ] {
            assert!(holds(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_as_part_of_a_word() {
        let k = Unsafe::KEYWORD;
        for code in [format!("let {k}ty = 1;"), format!("fn is_{k}() {{}}"), format!("let r#{k}x = 1;")] {
            assert!(!holds(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_in_comments() {
        let k = Unsafe::KEYWORD;
        for code in
            [format!("// {k} {{ }}"), format!("/* {k} {{ }} */"), format!("/* /* */ {k} */"), format!("/// {k}")]
        {
            assert!(!holds(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_in_literals() {
        let k = Unsafe::KEYWORD;
        for code in [
            format!("const CSP: &str = \"script-src '{k}-inline'\";"),
            format!("let s = \"\\\" {k} {{\";"),
            format!("let s = b\"{k}\";"),
            format!("let s = r\"{k} {{\";"),
            format!("let s = r#\"a \" {k} {{ \"#;"),
            format!("let s = br##\"\"# {k}\"##;"),
            format!("let s = c\"{k}\";"),
        ] {
            assert!(!holds(&code), "{code}");
        }
    }

    #[test]
    fn the_lint_name_in_a_literal_is_not_code() {
        let code = format!("let s = \"{}\";", Unsafe::LINT);
        assert!(!holds(&code), "{code}");
    }

    #[test]
    fn unbalanced_text_is_refused() {
        assert!(Unsafe::holds("fn f( {").is_err());
    }

    #[test]
    fn a_directory_over_the_limit_reports_or_fails_by_mode() {
        let mut workspace = Workspace::sample(vec![], &[]);
        workspace.entries = [
            "a",
            "a/1",
            "a/2",
            "a/3",
            "a/4",
            "b",
            "b/1",
            "t",
            "t/tests",
            "t/tests/1",
            "t/tests/2",
            "t/tests/3",
            "t/tests/4",
        ]
        .map(PathBuf::from)
        .to_vec();
        workspace.config.fanout.limit = 3;
        workspace.config.fanout.exempt = vec!["tests".to_owned()];
        let mut problems = Problems::default();
        Fanout.check(&workspace, &mut problems);
        assert_eq!(problems.crowded, [(PathBuf::from("a"), 4)]);
        assert_eq!(problems.found.len(), 0);

        workspace.config.fanout.mode = Strictness::Enforce;
        let found = Problems::of(&Fanout, &workspace);
        assert!(matches!(found.as_slice(), [Problem::Crowded { dir, entries: 4, limit: 3 }] if dir == Path::new("a")));
    }

    #[test]
    fn the_root_counts_its_directories_only() {
        let mut workspace = Workspace::sample(vec![], &[]);
        workspace.entries = ["a", "a/1", "b", "b/1", "c.md", "d.md", "e.md"].map(PathBuf::from).to_vec();
        workspace.config.fanout.limit = 2;
        let mut problems = Problems::default();
        Fanout.check(&workspace, &mut problems);
        assert_eq!(problems.crowded.len(), 0);
        workspace.config.fanout.limit = 1;
        Fanout.check(&workspace, &mut problems);
        assert_eq!(problems.crowded, [(PathBuf::new(), 2)]);
    }

    #[test]
    fn a_waived_directory_is_let_through() {
        let mut workspace = Workspace::sample(vec![], &[]);
        workspace.entries = ["docs", "docs/1", "docs/2", "docs/3"].map(PathBuf::from).to_vec();
        let text = "limit = 2\nmode = \"report\"\nexempt = []\n[[waive]]\npath = \"docs\"\nreason = \"prose\"\n";
        workspace.config.fanout = toml::from_str(text).unwrap();
        let mut problems = Problems::default();
        Fanout.check(&workspace, &mut problems);
        assert_eq!(problems.crowded.len(), 0);
    }
}
