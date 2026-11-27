//! `ink` on every folder under `fixture/site/`, in process: each build compared with
//! `fixture/site/<name>.snap` (the exit code, the diagnostics, the manifest and the HTML of every
//! page), `ink explain` on each compared with `fixture/explain/<name>.snap`, and the M1 exit
//! tests: a build twice is the same deploy, a pin of a decision's current value builds the same
//! deploy again, and no reader page runs script.
//!
//! The blog has code, so tests that build it compile a tree-sitter grammar and carry `highlight`
//! in their name for its budget (`.config/nextest.toml`).

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

/// The cache every build here shares, under target/ so it outlives the run and never lands in
/// a fixture: after the first run the blog's photos are taken from it, not encoded.
fn cache() -> String {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-cache").to_str().unwrap().to_owned()
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
    let (exit, stdout, stderr) =
        ink(&["build", "-C", dir.to_str().unwrap(), "-o", out.to_str().unwrap(), "--base", BASE, "--cache", &cache()]);
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
    let (exit, stdout, stderr) = ink(&[
        "build",
        "-C",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--base",
        BASE,
        "--json",
        "--cache",
        &cache(),
    ]);
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

/// `ink pin` writes the decision's current value into `ink.toml`, keeping what was there, and
/// the site builds the same deploy again: a pin of what inference already says changes nothing.
#[test]
fn a_pin_of_the_current_value_changes_no_deploy() {
    let dir = scratch("pin/site");
    copy(&fixtures().join("readme"), &dir);
    let mine = "# mine\nexclude = [\"drafts/**\"]\n";
    std::fs::write(dir.join("ink.toml"), mine).unwrap();
    let before = build(&dir, &scratch("pin/before"));
    let dir_text = dir.to_str().unwrap();
    let (exit, dry, stderr) = ink(&["pin", "usage.md", "page.title", "--dry-run", "-C", dir_text]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert_eq!(dry, "[page.\"usage.md\"]\ntitle = \"Usage\"\ndry run: nothing written\n");
    assert_eq!(std::fs::read_to_string(dir.join("ink.toml")).unwrap(), mine);
    let (exit, _, stderr) = ink(&["pin", "usage.md", "page.title", "-C", dir_text]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    let (exit, _, stderr) = ink(&["pin", "site.title", "-C", dir_text]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    let toml = std::fs::read_to_string(dir.join("ink.toml")).unwrap();
    assert!(toml.starts_with(mine), "{toml}");
    assert!(toml.contains("[page.\"usage.md\"]\ntitle = \"Usage\""), "{toml}");
    assert!(toml.contains("[site]\ntitle = \"My tool\""), "{toml}");
    let (_, why, _) = ink(&["explain", "usage.md", "--why", "page.title", "-C", dir_text]);
    assert!(why.starts_with("page.title on usage.md: “Usage” pinned in ink.toml"), "{why}");
    assert_eq!(build(&dir, &scratch("pin/after")), before);
}

#[test]
fn pin_refuses_a_code_of_the_other_scope_or_an_unknown_file() {
    let dir = fixtures().join("readme");
    let dir_text = dir.to_str().unwrap();
    for (args, said) in [
        (&["pin", "page.title"][..], "error: `page.title` is decided per file: give the file's path before it\n"),
        (
            &["pin", "usage.md", "site.title"][..],
            "error: `site.title` is decided once for the site, not on usage.md: give the code alone\n",
        ),
        (&["pin", "nosuch.md", "page.title"][..], "error: nosuch.md is not a file of the folder\n"),
        (&["pin", "page.nope"][..], "error: no decision has that code\n  caused by: `page.nope` is no decision code\n"),
    ] {
        let args: Vec<&str> = args.iter().copied().chain(["--dry-run", "-C", dir_text]).collect();
        let (exit, stdout, stderr) = ink(&args);
        assert_eq!((exit, stdout.as_str(), stderr.as_str()), (Exit::Usage, "", said), "{args:?}");
    }
}

#[test]
fn explain_why_replays_one_ladder_rung_by_rung() {
    let dir = fixtures().join("blog");
    let (exit, why, stderr) = ink(&["explain", "posts/later.md", "--why", "page.title", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert_eq!(
        why,
        "page.title on posts/later.md: “Later” from the name of posts/later.md, rung 4 of 4
   1 ink.toml           nothing
   2 front matter       nothing
   3 first heading      nothing
   4 file name          “Later”                  from the name of posts/later.md, chosen
to pin it: ink pin posts/later.md page.title
"
    );
    let (_, why, _) = ink(&["explain", "index.md", "--why", "page.summary", "-C", dir.to_str().unwrap()]);
    assert!(why.contains("   3 first paragraph    “Welcome."), "the outranked rung is shown: {why}");
    assert!(why.lines().nth(3).unwrap().ends_with(", outranked"), "{why}");
    let (_, json, _) = ink(&["explain", "--why", "site.layout", "-C", dir.to_str().unwrap(), "--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema"], "uses.trace/v1");
    assert_eq!(json["trace"]["code"], "site.layout");
    assert_eq!(json["trace"]["rungs"][3]["witness"], serde_json::json!({ "folder": "posts" }));
    assert_eq!(json["trace"]["rungs"][3]["heard"][0]["chosen"], true);
}

/// `name` built twice into one output folder: the second build compares with the first's
/// record, finds no change and no warning, and is the same deploy (the M1 exit check as 0042
/// restates it). One test per fixture, so each stays within its budget.
fn rebuilt_unchanged(name: &str) {
    let dir = fixtures().join(name);
    let out = scratch(&format!("rebuilt/{name}"));
    let args = [
        "build",
        "-C",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--base",
        BASE,
        "--json",
        "--cache",
        &cache(),
    ];
    let (exit, first, stderr) = ink(&args);
    assert_eq!(exit, Exit::Done, "{stderr}");
    let first: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first["changes"]["against"], "nothing", "{name}");
    let (exit, second, stderr) = ink(&args);
    assert_eq!(exit, Exit::Done, "{stderr}");
    let second: serde_json::Value = serde_json::from_str(&second).unwrap();
    assert_eq!(second["deploy"], first["deploy"], "{name}");
    assert_eq!(second["changes"]["against"], serde_json::json!({ "deploy": { "deploy": first["deploy"] } }), "{name}");
    let changes = &second["changes"];
    for list in ["public", "withdrawn", "moved", "decisions"] {
        assert_eq!(changes[list], serde_json::json!([]), "{name}: {list} in {changes}");
    }
    assert_eq!(second["warnings"], first["warnings"], "{name}: no engine warning");
}

#[test]
fn highlight_the_blog_built_again_changes_nothing() {
    rebuilt_unchanged("blog");
}

#[test]
fn broken_frontmatter_built_again_changes_nothing() {
    rebuilt_unchanged("broken-frontmatter");
}

#[test]
fn the_docs_built_again_change_nothing() {
    rebuilt_unchanged("docs");
}

#[test]
fn the_notes_built_again_change_nothing() {
    rebuilt_unchanged("notes");
}

#[test]
fn the_readme_built_again_changes_nothing() {
    rebuilt_unchanged("readme");
}

#[test]
fn every_site_that_builds_is_built_again() {
    assert_eq!(BUILDS, ["blog", "broken-frontmatter", "docs", "notes", "readme"]);
}

#[test]
fn a_build_lists_what_changed_since_the_last_newly_public_first() {
    let dir = scratch("changed/site");
    copy(&fixtures().join("readme"), &dir);
    let out = scratch("changed/out");
    build(&dir, &out);
    std::fs::write(dir.join("usage.md"), "# How to use it\n\nRun it.\n").unwrap();
    std::fs::write(dir.join("new.md"), "# New\n\nHello.\n").unwrap();
    let args = ["build", "-C", dir.to_str().unwrap(), "-o", out.to_str().unwrap(), "--base", BASE, "--cache", &cache()];
    let (exit, stdout, stderr) = ink(&args);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert_eq!(stdout.trim().len(), 64, "stdout is the deploy id alone: {stdout}");
    let lines: Vec<&str> = stderr.lines().collect();
    assert!(lines[0].starts_with("against the previous build in "), "{stderr}");
    assert_eq!(lines[1..3], ["newly public", "  new.md                           /new"], "{stderr}");
    assert!(
        stderr.contains(
            "  usage.md                         page.title “Usage” -> “How to use it”  content: the file was edited"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("  new.md                           new, content: the file is new"), "{stderr}");
    assert!(stderr.contains("site.nav"), "the navigation follows the new page: {stderr}");
    assert!(lines.last().unwrap().starts_with("1 newly public, 0 no longer public, 0 moved, "), "{stderr}");
}

/// `site.format` is a pin, then the `.obsidian/` marker, then markdown, each replayed by `--why`,
/// and a pin is said.
#[test]
fn the_format_is_a_pin_then_the_vault_marker_then_markdown() {
    let why = |name: &str| {
        let dir = fixtures().join(name);
        let (exit, why, stderr) = ink(&["explain", "--why", "site.format", "-C", dir.to_str().unwrap(), "-v"]);
        assert_eq!(exit, Exit::Done, "{stderr}");
        (why, stderr)
    };
    let (why_vault, said) = why("vault");
    assert_eq!(
        why_vault,
        "site.format on the site: `ink:format-obsidian` because .obsidian is there, rung 2 of 3
   1 ink.toml           nothing
   2 .obsidian folder   `ink:format-obsidian`    because .obsidian is there, chosen
   3 default            nothing
to pin it: ink pin site.format
"
    );
    assert!(said.contains("note[format.obsidian.chosen]"), "{said}");
    let (plain, said) = why("readme");
    assert!(plain.starts_with("site.format on the site: `ink:format-markdown` by default, rung 3 of 3"), "{plain}");
    assert!(!said.contains("format."), "markdown by default is not said: {said}");
    let (pinned, said) = why("vault-markdown");
    assert_eq!(
        pinned,
        "site.format on the site: `ink:format-markdown` pinned in ink.toml, bytes 7..37, rung 1 of 3
   1 ink.toml           `ink:format-markdown`    pinned in ink.toml, bytes 7..37, chosen
   2 .obsidian folder   `ink:format-obsidian`    because .obsidian is there, outranked
   3 default            nothing
to pin it: ink pin site.format
"
    );
    assert!(said.contains("note[format.pin.chosen]: `ink.toml` pins `site.format` to `ink:format-markdown`"), "{said}");
    assert!(!said.contains("format.obsidian.chosen"), "{said}");
    let (pinned, said) = why("notes-obsidian");
    assert!(pinned.starts_with("site.format on the site: `ink:format-obsidian` pinned in ink.toml"), "{pinned}");
    assert!(said.contains("note[format.pin.chosen]: `ink.toml` pins `site.format` to `ink:format-obsidian`"), "{said}");
}

/// A pinned format reads the files: a vault pinned to markdown leaves `#todo` text and a folder
/// pinned to Obsidian reads it as a tag; `ink pin site.format` writes what is there.
#[test]
fn a_pinned_format_reads_the_markdown_and_ink_pin_writes_it() {
    let tags = |name: &str| {
        let dir = fixtures().join(name);
        let (exit, json, stderr) = ink(&["explain", "-C", dir.to_str().unwrap(), "--json"]);
        assert_eq!(exit, Exit::Done, "{stderr}");
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = json["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "page.tags" && d["subject"]["file"] == "index.md")
            .cloned();
        tags.unwrap()
    };
    assert_eq!(tags("vault-markdown")["value"], serde_json::json!([]));
    let obsidian = tags("notes-obsidian");
    assert_eq!(obsidian["value"][0]["written"], "todo");
    assert_eq!(obsidian["evidence"]["inline"]["path"], "index.md", "{obsidian}");
    let dir = scratch("pin-format/site");
    copy(&fixtures().join("vault"), &dir);
    let (exit, written, stderr) = ink(&["pin", "site.format", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(written.starts_with("[site]\nformat = \"ink:format-obsidian\"\n"), "{written}");
    assert_eq!(std::fs::read_to_string(dir.join("ink.toml")).unwrap(), "[site]\nformat = \"ink:format-obsidian\"\n");
    let (exit, _, stderr) = ink(&["explain", "--why", "site.format", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Done, "{stderr}");
}

#[test]
fn a_format_this_ink_does_not_hold_stops_the_command() {
    let dir = scratch("format-unknown/site");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ink.toml"), "[site]\nformat = \"acme:format-wiki\"\n").unwrap();
    std::fs::write(dir.join("index.md"), "# Home\n").unwrap();
    let (exit, _, stderr) = ink(&["check", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
    assert!(
        stderr.starts_with("error: `site.format` names `acme:format-wiki`, which this ink does not hold"),
        "{stderr}"
    );
    std::fs::write(dir.join("ink.toml"), "[site]\nformat = \"quiet\"\n").unwrap();
    let (exit, _, stderr) = ink(&["check", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Content, "{stderr}");
    assert!(stderr.contains("config.value.invalid"), "{stderr}");
}

#[test]
fn check_exits_by_what_it_found() {
    let check = |name: &str, strict: &[&str]| {
        let dir = fixtures().join(name);
        let args: Vec<&str> =
            ["check", "-C", dir.to_str().unwrap()].into_iter().chain(strict.iter().copied()).collect();
        ink(&args)
    };
    let (exit, stdout, _) = check("readme", &[]);
    assert_eq!(exit, Exit::Done);
    assert_eq!(stdout, "2 pages, 30 decisions, 0 errors, 0 warnings\n");
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
        stdout.contains("page.title         “Usage”                  from the heading of usage.md, bytes 0..7"),
        "{stdout}"
    );
    let (_, json, _) = ink(&["explain", "-C", dir.to_str().unwrap(), "--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema"], "uses.explain/v2");
    assert!(json["decisions"].as_array().unwrap().len() > 10);
    let format = &json["decisions"][0];
    assert_eq!(format["code"], "site.format", "decisions are tagged by their code: {format}");
    assert_eq!((format["subject"].as_str(), format["value"].as_str()), (Some("site"), Some("ink:format-markdown")));
    let root = &json["decisions"][1];
    assert_eq!(root["code"], "site.root", "decisions are tagged by their code: {root}");
    assert_eq!(root["subject"], "site");
    assert_eq!(root["value"], "top");
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
    let (exit, _, stderr) = ink(&["build", "-C", dir.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(exit, Exit::Usage);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert_eq!(std::fs::read_to_string(out.join("keep.txt")).unwrap(), "mine");
    let (exit, _, stderr) = ink(&["build", "-C", dir.to_str().unwrap(), "--base", "ftp://x"]);
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

#[test]
fn no_command_reads_a_custom_output_folder_as_content() {
    let dir = scratch("custom-out/site");
    copy(&fixtures().join("readme"), &dir);
    let dir_text = dir.to_str().unwrap();
    let (_, before, _) = ink(&["check", "-C", dir_text]);
    let (_, explained, _) = ink(&["explain", "-C", dir_text]);
    build(&dir, &dir.join("public"));
    let (exit, after, _) = ink(&["check", "-C", dir_text]);
    assert_eq!(exit, Exit::Done);
    assert_eq!(after, before);
    assert_eq!(ink(&["explain", "-C", dir_text]).1, explained);
    let (_, _, said) = ink(&["check", "-C", dir_text, "-v"]);
    assert!(said.contains("note[source.output.skipped]: public holds the build's own output"), "{said}");
    let preview = cli::Preview::of(&dir, BASE).unwrap();
    assert!(preview.is_some(), "the preview builds without reading public/");
}

#[test]
fn a_missing_or_unlistable_folder_is_a_usage_error() {
    let missing = scratch("missing-folder");
    let file = fixtures().join("readme/README.md");
    for dir in [missing.to_str().unwrap(), file.to_str().unwrap()] {
        for command in ["check", "build"] {
            let (exit, _, stderr) = ink(&[command, "-C", dir]);
            assert_eq!(exit, Exit::Usage, "{command} {dir}: {stderr}");
            assert!(stderr.starts_with("error[source.root.unreadable]"), "{stderr}");
        }
        let (exit, _, stderr) = ink(&["explain", "-C", dir]);
        assert_eq!(exit, Exit::Usage, "{stderr}");
    }
    assert!(!missing.exists());
}

#[test]
fn explain_refuses_a_path_that_is_no_file() {
    let dir = fixtures().join("readme");
    let (exit, stdout, stderr) = ink(&["explain", "-C", dir.to_str().unwrap(), "nosuch.md"]);
    assert_eq!(exit, Exit::Usage);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "error: nosuch.md is not a file of the folder\n");
    let (exit, _, _) = ink(&["explain", "nosuch.md", "--why", "page.title", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Usage);
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

#[test]
fn highlight_a_cache_changes_no_byte_of_the_deploy() {
    let cache = scratch("own-cache");
    let cache_text = cache.to_str().unwrap();
    let dir = fixtures().join("blog");
    let out = scratch("own-cache-out");
    let args = [
        "build",
        "-C",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--base",
        BASE,
        "--cache",
        cache_text,
        "-v",
    ];
    let (exit, cold, said) = ink(&args);
    assert_eq!(exit, Exit::Done, "{said}");
    assert!(said.contains("note[render.cache.cold]"), "{said}");
    let (_, warm, said) = ink(&args);
    assert_eq!(warm, cold);
    assert!(!said.contains("render.cache.cold"), "{said}");
    for value in std::fs::read_dir(cache.join("value")).unwrap() {
        let path = value.unwrap().path();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.push(0);
        std::fs::write(&path, bytes).unwrap();
    }
    let (_, rebuilt, said) = ink(&args);
    assert_eq!(rebuilt, cold);
    assert!(said.contains("note[cache.entry.corrupt]"), "{said}");
    assert!(!dir.join(".ink").exists(), "the fixture holds no cache");
}
