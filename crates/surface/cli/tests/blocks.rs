//! The output of real commands as goldens (`fixture/blocks/`): the explain tree, the ladder, the
//! select table, the pin patch, diagnostics with a snippet, errors, a change set and the usage
//! error, each as text without colour, with colour (escape written `\e`), with ascii glyphs and at
//! 40 columns, and the error and check documents as JSON.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_cli as cli;

use std::path::{Path, PathBuf};

use cli::Surroundings;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site")
}

fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-blocks").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() { copy(&path, &target) } else { std::fs::copy(&path, &target).map(|_| ()).unwrap() }
    }
}

/// One way the output looks: the process it runs in, and the arguments that go with it.
struct Look {
    name: &'static str,
    surroundings: Surroundings,
    flags: &'static [&'static str],
}

impl Look {
    fn all() -> [Self; 4] {
        let utf8 = [("LANG", "en_US.UTF-8")];
        let with = |extra: &[(&str, &str)]| {
            let vars: Vec<(&str, &str)> = utf8.iter().chain(extra).copied().collect();
            Surroundings::declared(&vars, false, false)
        };
        [
            Self { name: "plain", surroundings: Surroundings::empty(), flags: &[] },
            Self { name: "colour", surroundings: Surroundings::empty(), flags: &["--color", "always"] },
            Self { name: "ascii", surroundings: with(&[("INK_GLYPHS", "ascii")]), flags: &[] },
            Self { name: "narrow", surroundings: with(&[("INK_WIDTH", "40")]), flags: &[] },
        ]
    }

    /// `ink <args>` in this look: stdout, then stderr, as one text.
    fn ink(&self, args: &[&str]) -> String {
        let all = std::iter::once("ink").chain(self.flags.iter().copied()).chain(args.iter().copied());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        cli::Cli::start_in(&self.surroundings, all, &mut out, &mut err).unwrap();
        let (out, err) = (String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap());
        let mut text = String::new();
        if !out.is_empty() {
            text.push_str("# stdout\n");
            text.push_str(&out);
        }
        if !err.is_empty() {
            text.push_str("# stderr\n");
            text.push_str(&err);
        }
        text.replace('\x1b', "\\e")
    }
}

#[test]
fn every_block_of_a_command_looks_as_expected() {
    let site = |name: &str| {
        let copied = scratch(&format!("sites/{name}"));
        copy(&fixtures().join(name), &copied);
        copied.to_str().unwrap().to_owned()
    };
    let mut cases: Vec<(String, String)> = Vec::new();
    for look in Look::all() {
        let readme = site("readme");
        let vault = site("vault");
        let named = |command: &str| format!("{command}-{}", look.name);
        cases.push((named("explain"), look.ink(&["explain", "-C", &readme])));
        cases.push((named("why"), look.ink(&["explain", "usage.md", "--why", "page.title", "-C", &readme])));
        cases.push((named("select"), look.ink(&["select", "-C", &vault])));
        cases.push((named("pin"), look.ink(&["pin", "usage.md", "page.title", "--dry-run", "-C", &readme])));
        cases.push((named("check-clean"), look.ink(&["check", "-C", &readme])));
        cases.push((named("check-error"), look.ink(&["check", "-C", &site("ambiguous")])));
        cases.push((named("check-warning"), look.ink(&["check", "-C", &site("broken-frontmatter")])));
        cases.push((named("error"), look.ink(&["explain", "-C", &readme, "nosuch.md"])));
        cases.push((named("usage"), look.ink(&["explain", "--wy"])));
        cases.push((named("changes"), Changes::of(&look)));
    }
    let json = Look::all().into_iter().next().unwrap();
    cases.push(("error-json".to_owned(), json.ink(&["pin", "page.nope", "-C", &site("readme"), "--json"])));
    cases.push(("usage-json".to_owned(), json.ink(&["explain", "--wy", "--json"])));
    cases.push(("check-json".to_owned(), json.ink(&["check", "-C", &site("ambiguous"), "--json"])));
    fixture::cases("fixture/blocks", cases);
}

/// A second build, from the change set down: the lines before it and after it hold times and the
/// output folder, which change from run to run.
struct Changes;

impl Changes {
    fn of(look: &Look) -> String {
        let dir = scratch(&format!("changes-{}/site", look.name));
        let out = scratch(&format!("changes-{}/out", look.name));
        copy(&fixtures().join("readme"), &dir);
        let cache = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-cache").join("blocks");
        let args = |extra: &[&'static str]| -> Vec<String> {
            let mut args = vec!["build".to_owned(), "-C".to_owned(), dir.to_str().unwrap().to_owned()];
            args.extend(
                ["-o", out.to_str().unwrap(), "--base", "https://ink.example", "--cache", cache.to_str().unwrap()]
                    .map(ToOwned::to_owned),
            );
            args.extend(extra.iter().map(|arg| (*arg).to_owned()));
            args
        };
        let run = |args: Vec<String>| {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            look.ink(&args)
        };
        run(args(&[]));
        std::fs::write(dir.join("usage.md"), "# How to use it\n\nRun it.\n").unwrap();
        std::fs::write(dir.join("new.md"), "# New\n\nHello.\n").unwrap();
        let second = run(args(&[]));
        let from = second
            .lines()
            .position(|line| line.contains("Changes"))
            .unwrap_or_else(|| panic!("no change set in {second}"));
        let lines: Vec<&str> = second.lines().skip(from).take_while(|line| !line.contains("Built")).collect();
        let masked: Vec<String> = lines
            .iter()
            .map(|line| match line.split_once("against deploy ") {
                Some((head, _)) => format!("{head}against deploy <id>"),
                None => (*line).to_owned(),
            })
            .collect();
        masked.join("\n") + "\n"
    }
}

#[test]
fn colour_is_a_flag_away_from_a_pipe() {
    let colour = Look::all().into_iter().nth(1).unwrap();
    assert!(colour.ink(&["check", "-C", fixtures().join("readme").to_str().unwrap()]).contains("\\e["));
}
