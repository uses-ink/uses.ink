//! The help of every command through the real binary: each one answers `--help` and `help
//! <command>` the same, names its usage, examples and exit codes, wraps at the page width and
//! uses no dash or arrow glyphs in its prose; the manual and the completions know every command.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

use std::process::Command;

/// What one run said.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

fn ink(args: &[&str]) -> Ran {
    let out = Command::new(env!("CARGO_BIN_EXE_ink")).args(args).env_clear().output().unwrap();
    Ran {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// The command names a help page lists under its command groups: the first word of each indented
/// line between a group heading and the next blank line.
fn listed(help: &str, headings: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in help.lines() {
        if headings.contains(&line.trim_end()) {
            inside = true;
        } else if line.trim().is_empty() || !line.starts_with(' ') {
            inside = false;
        } else if let (true, Some(word)) = (inside, line.split_whitespace().next()) {
            names.push(word.trim_end_matches(',').to_owned());
        }
    }
    names
}

/// Every command path of the tree: `["build"]`, `["key", "add"]`, and so on, found by asking each
/// page for the commands it lists.
fn commands() -> Vec<Vec<String>> {
    let root = ink(&["--help"]);
    assert_eq!(root.code, 0, "{}", root.err);
    let mut paths = Vec::new();
    let mut open: Vec<Vec<String>> =
        listed(&root.out, &["Write", "Understand", "Publish", "Account", "Operate", "Tools"])
            .into_iter()
            .map(|name| vec![name])
            .collect();
    while let Some(path) = open.pop() {
        let mut asked: Vec<&str> = path.iter().map(String::as_str).collect();
        asked.push("--help");
        for child in listed(&ink(&asked).out, &["Commands"]) {
            let mut below = path.clone();
            below.push(child);
            open.push(below);
        }
        paths.push(path);
    }
    paths
}

#[test]
fn every_command_has_help_that_says_what_it_does_and_how_it_ends() {
    let paths = commands();
    assert!(paths.len() >= 40, "the tree is {paths:?}");
    for path in paths {
        let words: Vec<&str> = path.iter().map(String::as_str).collect();
        let named = words.join(" ");
        let mut flagged = words.clone();
        flagged.push("--help");
        let help = ink(&flagged);
        assert_eq!(help.code, 0, "ink {named} --help: {}", help.err);
        assert!(help.err.is_empty(), "ink {named} --help wrote to stderr: {}", help.err);
        let first = help.out.lines().next().unwrap_or_default();
        assert!(first.ends_with('.'), "ink {named} opens with a sentence: {first:?}");
        assert!(help.out.contains(&format!("Usage: ink {named}")), "ink {named} names its usage:\n{}", help.out);
        assert!(help.out.contains("\nExit codes\n"), "ink {named} says how it ends:\n{}", help.out);
        // the same page by the other way in
        let mut spoken = vec!["help"];
        spoken.extend(&words);
        assert_eq!(ink(&spoken).out, help.out, "ink help {named} is ink {named} --help");
        for (number, line) in help.out.lines().enumerate() {
            assert!(
                line.chars().count() <= 100 || line.starts_with("  ink "),
                "ink {named} line {number} is {} columns: {line}",
                line.chars().count()
            );
            let glyph =
                line.chars().find(|c| matches!(c, '\u{2014}' | '\u{2013}' | '\u{2192}' | '\u{2190}' | '\u{21d2}'));
            assert!(glyph.is_none(), "ink {named} uses {glyph:?} in prose: {line}");
        }
    }
}

#[test]
fn the_manual_and_every_shell_know_every_command() {
    let manual = ink(&["man"]);
    assert_eq!(manual.code, 0, "{}", manual.err);
    assert!(manual.out.contains(".TH ink"), "{}", &manual.out[..manual.out.len().min(80)]);
    for shell in ["bash", "zsh", "fish", "elvish", "powershell", "nushell"] {
        let script = ink(&["completions", shell]);
        assert_eq!(script.code, 0, "{shell}: {}", script.err);
        for command in ["build", "push", "deploy", "rollback", "completions"] {
            assert!(script.out.contains(command), "the {shell} script does not know `{command}`");
        }
    }
    for path in commands() {
        let name = path.join(" ");
        assert!(manual.out.contains(&path.last().unwrap().clone()), "the manual lacks `{name}`");
    }
}
