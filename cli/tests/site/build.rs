//! `ink build` and its output folder: the golden of every site, the same deploy twice, and what a
//! build refuses to write over.

use std::fmt::Write as _;

use super::*;

/// Fixtures that build; the others fail on purpose.
const BUILDS: &[&str] = &["blog", "broken-frontmatter", "docs", "notes", "readme"];

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
    for line in
        stderr.lines().filter(|line| !line.contains(r#""severity":"notice""#) && !line.contains("uses.progress/v1"))
    {
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
    assert!(stderr.contains("Changes  1 newly public, 0 no longer public, 0 moved, "), "{stderr}");
    assert!(stderr.contains(" decisions changed, against deploy "), "{stderr}");
    assert!(stderr.contains("newly public (1)\n  +  new.md  /new\n"), "{stderr}");
    assert!(stderr.contains("page.title"), "{stderr}");
    assert!(stderr.contains("“Usage” → “How to use it”"), "{stderr}");
    assert!(stderr.contains("content: the file was edited"), "{stderr}");
    assert!(stderr.contains("content: the file is new"), "{stderr}");
    assert!(stderr.contains("site.nav"), "the navigation follows the new page: {stderr}");
    assert!(stderr.lines().any(|line| line.trim_start().starts_with("Built  ")), "{stderr}");
}

#[test]
fn a_build_refuses_a_folder_it_did_not_write() {
    let out = scratch("foreign");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("keep.txt"), "mine").unwrap();
    let dir = fixtures().join("readme");
    let (exit, _, stderr) = ink(&["build", "-C", dir.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(exit, Exit::Usage);
    assert!(stderr.starts_with("error[output.folder.foreign]: "), "{stderr}");
    assert!(stderr.ends_with("  nothing was changed\n"), "{stderr}");
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
    let (_, _, before) = ink(&["check", "-C", dir_text]);
    let (_, explained, _) = ink(&["explain", "-C", dir_text]);
    build(&dir, &dir.join("public"));
    let (exit, _, after) = ink(&["check", "-C", dir_text]);
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
