//! `ink` on every folder under `fixture/site/`, in process: each build compared with
//! `fixture/site/<name>.snap` (the exit code, the diagnostics, the manifest and the HTML of every
//! page), `ink explain` on each compared with `fixture/explain/<name>.snap`, and the M1 exit
//! tests: a build twice is the same deploy, `ink explain --pin` written as `ink.toml` builds the
//! same deploy again, and no reader page runs script.
//!
//! The blog has code, so tests that build it load giallo's grammars and carry `highlight` in their
//! name for its budget (`.config/nextest.toml`).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "tests stop at the first surprise"
)]

extern crate uses_ink_cli as cli;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use clap::Parser;
use cli::{Cli, Exit};

/// Fixtures that build; the others fail on purpose.
const BUILDS: &[&str] = &["blog", "broken-frontmatter", "docs", "notes", "readme"];
const BASE: &str = "https://ink.example";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site")
}

/// A fresh scratch folder for `name`.
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    path
}

/// `ink <args>`: the exit, stdout and stderr.
fn ink(args: &[&str]) -> (Exit, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("ink").chain(args.iter().copied())).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = cli.run(&mut out, &mut err);
    (exit, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// Builds `dir` into `out` and returns the deploy id.
fn build(dir: &Path, out: &Path) -> String {
    let (exit, stdout, stderr) = ink(&["build", dir.to_str().unwrap(), "-o", out.to_str().unwrap(), "--base", BASE]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    stdout.trim().to_owned()
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() { copy(&path, &target) } else { std::fs::copy(&path, &target).map(|_| ()).unwrap() }
    }
}

/// Every file under `dir`, relative to it, sorted.
fn files(dir: &Path, base: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, base, found);
        } else {
            found.push(path.strip_prefix(base).unwrap().to_path_buf());
        }
    }
    found.sort();
}

/// What no reader page may hold: a script element, an `on*=` handler or a `javascript:` URL.
fn refuse_script(page: &Path, html: &str) {
    let lower = html.to_ascii_lowercase();
    assert!(!lower.contains("<script"), "{} has a script element", page.display());
    assert!(!lower.contains("javascript:"), "{} has a javascript: URL", page.display());
    for tag in lower.split('<').skip(1).filter_map(|rest| rest.split('>').next()) {
        // attribute names only: every other piece between quotes is a value
        let names: String = tag.split('"').step_by(2).collect::<Vec<_>>().join(" ");
        let handler = names.split_whitespace().skip(1).any(|attribute| {
            attribute.starts_with("on") && attribute.split('=').next().is_some_and(|name| name.len() > 2)
        });
        assert!(!handler, "{} has an event handler in <{tag}>", page.display());
    }
}

/// The exit, the diagnostics (not notices: they hold file times), every entry written, and the
/// text of every HTML page.
fn report(name: &str) -> String {
    let out = scratch(&format!("report/{name}"));
    let dir = fixtures().join(name);
    let (exit, stdout, stderr) =
        ink(&["build", dir.to_str().unwrap(), "-o", out.to_str().unwrap(), "--base", BASE, "--json"]);
    let mut report = String::new();
    let _infallible = writeln!(report, "# exit {}", exit.code());
    for line in stderr.lines().filter(|line| !line.contains(r#""severity":"notice""#)) {
        let _infallible = writeln!(report, "{line}");
    }
    let _infallible = writeln!(report, "# result\n{}", stdout.replace(out.to_str().unwrap(), "<out>").trim());
    if !out.join(".ink-manifest.json").is_file() {
        return report;
    }
    let mut found = Vec::new();
    files(&out, &out, &mut found);
    let _infallible = writeln!(report, "# files");
    for file in &found {
        let _infallible = writeln!(report, "{}", file.display());
    }
    for file in found.iter().filter(|file| file.extension().is_some_and(|e| e == "html")) {
        let html = std::fs::read_to_string(out.join(file)).unwrap();
        refuse_script(file, &html);
        let _infallible = writeln!(report, "## {}\n{}", file.display(), html.replace("><", ">\n<"));
    }
    report
}

/// The name of every site under `fixture/site/`, sorted.
fn sites() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert!(names.len() >= 6, "the fixtures are missing: {names:?}");
    names
}

#[test]
fn highlight_sites_build_as_expected() {
    for name in sites() {
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path(fixtures());
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        settings.bind(|| insta::assert_snapshot!(name.clone(), report(&name)));
    }
}

#[test]
fn highlight_a_build_twice_is_the_same_deploy() {
    for name in BUILDS {
        let dir = fixtures().join(name);
        let first = build(&dir, &scratch(&format!("twice/{name}/a")));
        let second = build(&dir, &scratch(&format!("twice/{name}/b")));
        assert_eq!(first, second, "{name}");
        assert_eq!(first.len(), 64, "{name}: {first}");
    }
}

#[test]
fn highlight_pins_written_back_build_the_same_deploy() {
    for name in BUILDS {
        let dir = scratch(&format!("pin/{name}/site"));
        copy(&fixtures().join(name), &dir);
        let before = build(&dir, &scratch(&format!("pin/{name}/before")));
        let (exit, pins, stderr) = ink(&["explain", "--pin", "-C", dir.to_str().unwrap()]);
        assert_eq!(exit, Exit::Done, "{stderr}");
        // Codes config cannot write yet are listed as `# not pinned`; they decide the same again.
        std::fs::write(dir.join("ink.toml"), &pins).unwrap();
        let after = build(&dir, &scratch(&format!("pin/{name}/after")));
        assert_eq!(before, after, "{name}: the pins of\n{pins}");
    }
}

#[test]
fn check_exits_by_what_it_found() {
    let check = |name: &str, strict: &[&str]| {
        let dir = fixtures().join(name);
        let args: Vec<&str> = ["check", dir.to_str().unwrap()].into_iter().chain(strict.iter().copied()).collect();
        ink(&args)
    };
    let (exit, stdout, _) = check("readme", &[]);
    assert_eq!(exit, Exit::Done);
    assert_eq!(stdout, "2 pages, 29 decisions, 0 errors, 0 warnings\n");
    let (exit, _, stderr) = check("ambiguous", &[]);
    assert_eq!(exit, Exit::Content);
    assert!(stderr.starts_with("error[link.target.ambiguous]:"), "{stderr}");
    let (exit, _, stderr) = check("broken-frontmatter", &["--strict"]);
    assert_eq!(exit, Exit::Strict);
    assert!(stderr.contains("warning[frontmatter.syntax.invalid]"), "{stderr}");
    assert!(stderr.contains("2 | / title: [unclosed"), "the source line is under the diagnostic: {stderr}");
}

#[test]
fn explain_shows_decisions_with_evidence() {
    let dir = fixtures().join("readme");
    let (exit, stdout, _) = ink(&["explain", "-C", dir.to_str().unwrap(), "usage.md"]);
    assert_eq!(exit, Exit::Done);
    assert!(stdout.starts_with("usage.md\n"), "{stdout}");
    assert!(
        stdout.contains("page.title         Usage                    from the heading of usage.md, bytes 0..7"),
        "{stdout}"
    );
    let (_, json, _) = ink(&["explain", "-C", dir.to_str().unwrap(), "--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema"], "uses.explain/v1");
    assert!(json["decisions"].as_array().unwrap().len() > 10);
}

/// `ink explain` on every site, compared with `fixture/explain/<name>.snap`. Not its JSON: that
/// holds the file times the folder reports.
#[test]
fn explain_shows_every_site_as_expected() {
    let cases = sites().into_iter().map(|name| {
        let (_, lines, _) = ink(&["explain", "-C", fixtures().join(&name).to_str().unwrap()]);
        (name, lines)
    });
    fixture::cases("fixture/explain", cases);
}

#[test]
fn a_build_refuses_a_folder_it_did_not_write() {
    let out = scratch("foreign");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("keep.txt"), "mine").unwrap();
    let dir = fixtures().join("readme");
    let (exit, _, stderr) = ink(&["build", dir.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(exit, Exit::Usage);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert_eq!(std::fs::read_to_string(out.join("keep.txt")).unwrap(), "mine");
    let (exit, _, stderr) = ink(&["build", dir.to_str().unwrap(), "--base", "ftp://x"]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
}

#[test]
fn a_build_writes_over_its_own_output() {
    let out = scratch("again");
    let dir = fixtures().join("readme");
    let first = build(&dir, &out);
    assert_eq!(build(&dir, &out), first);
    assert!(out.join("usage/index.html").is_file());
    assert!(out.join("404.html").is_file());
}

/// A golden whose site folder is gone fails, so no stale golden stays behind.
#[test]
fn every_golden_has_a_site() {
    let orphans: Vec<PathBuf> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| !path.is_dir())
        .filter(|path| path.extension().is_none_or(|e| e != "snap") || !path.with_extension("").is_dir())
        .collect();
    assert!(orphans.is_empty(), "files in fixture/site/ that are not a site's golden: {orphans:?}");
}
