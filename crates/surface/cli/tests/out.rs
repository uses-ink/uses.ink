//! `ink build` and its output folder: it removes only what an earlier build wrote and still
//! holds those bytes, reaches nothing through a link, never clears the site's own folder, and a
//! failed build leaves the previous deploy as it was.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_cli as cli;

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use clap::Parser;
use cli::{Cli, Exit};

/// A fresh scratch folder for `name`, holding a copy of the `readme` site at `site/`.
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-out").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/readme");
    for file in ["README.md", "usage.md", "LICENSE", "src/main.rs"] {
        let to = path.join("site").join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(fixture.join(file), to).unwrap();
    }
    path
}

/// `ink <args>`: the exit, stdout and stderr.
fn ink(args: &[&Path]) -> (Exit, String, String) {
    let args = args.iter().map(|arg| arg.to_str().unwrap());
    let cli = Cli::try_parse_from(std::iter::once("ink").chain(args)).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = cli.run(&mut out, &mut err);
    (exit, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// `ink build <site> -o <out> [extra]`.
fn build(root: &Path, out: &Path, extra: &[&str]) -> (Exit, String, String) {
    let site = root.join("site");
    let mut args: Vec<&Path> = vec![Path::new("build"), Path::new("-C"), &site, Path::new("-o"), out];
    args.extend(extra.iter().map(Path::new));
    ink(&args)
}

/// Every file under `dir` with its bytes, sorted.
fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push((path.strip_prefix(dir).unwrap().to_path_buf(), std::fs::read(&path).unwrap()));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_forged_manifest_removes_nothing() {
    let root = scratch("forged");
    std::fs::create_dir_all(root.join("victim")).unwrap();
    std::fs::write(root.join("victim/secret.txt"), "secret").unwrap();
    let out = root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let hash = model::Hash::of(b"secret");
    let secret = root.join("victim/secret.txt");
    let forged = [
        r#"{"files":["../victim/secret.txt"]}"#.to_owned(),
        format!(r#"{{"files":[{{"path":"../victim/secret.txt","hash":"{hash}"}}]}}"#),
        format!(r#"{{"files":[{{"path":"{}","hash":"{hash}"}}]}}"#, secret.display()),
        format!(r#"{{"files":[{{"path":"","hash":"{hash}"}}]}}"#),
    ];
    for manifest in forged {
        std::fs::write(out.join(".ink-manifest.json"), &manifest).unwrap();
        let (exit, _, stderr) = build(&root, &out, &[]);
        assert_eq!(exit, Exit::Usage, "{manifest}: {stderr}");
        assert!(stderr.contains("does not parse"), "{manifest}: {stderr}");
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "secret", "{manifest}");
    }
}

#[test]
fn a_listed_file_whose_bytes_changed_is_refused_not_removed() {
    let root = scratch("changed");
    let out = root.join("out");
    assert_eq!(build(&root, &out, &[]).0, Exit::Done);
    std::fs::write(out.join("usage/index.html"), "mine now").unwrap();
    let before = tree(&out);
    let (exit, _, stderr) = build(&root, &out, &[]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
    assert!(stderr.contains("usage/index.html is not what the earlier `ink build` wrote there"), "{stderr}");
    assert_eq!(tree(&out), before, "nothing was removed");
}

#[test]
fn an_output_folder_that_is_or_holds_the_site_is_refused() {
    let root = scratch("holds");
    let site = root.join("site");
    let sources = tree(&site);
    let hash = model::Hash::of(std::fs::read(site.join("README.md")).unwrap());
    let forged = format!(r#"{{"files":[{{"path":"README.md","hash":"{hash}"}}]}}"#);
    std::fs::write(site.join(".ink-manifest.json"), &forged).unwrap();
    for out in [site.clone(), site.join("."), root.clone(), root.join("site/../site")] {
        let (exit, _, stderr) = build(&root, &out, &[]);
        assert_eq!(exit, Exit::Usage, "{}: {stderr}", out.display());
        assert!(stderr.contains("is or holds the site's folder"), "{stderr}");
    }
    std::fs::remove_file(site.join(".ink-manifest.json")).unwrap();
    assert_eq!(tree(&site), sources, "no source was removed");
}

#[test]
fn a_failed_build_writes_nothing_and_keeps_the_previous_deploy() {
    let root = scratch("failed");
    let out = root.join("out");
    assert_eq!(build(&root, &out, &[]).0, Exit::Done);
    let deployed = tree(&out);
    std::fs::write(root.join("site/w.md"), "---\ntitle: [\n---\n").unwrap();
    let (exit, stdout, stderr) = build(&root, &out, &["--strict"]);
    assert_eq!(exit, Exit::Strict, "{stderr}");
    assert_eq!(stdout, "", "no deploy is announced");
    assert_eq!(tree(&out), deployed, "a strict failure writes nothing");
    let fresh = root.join("fresh");
    assert_eq!(build(&root, &fresh, &["--strict"]).0, Exit::Strict);
    assert!(!fresh.exists(), "a strict failure does not even make the folder");
    std::fs::create_dir_all(root.join("site/one")).unwrap();
    std::fs::create_dir_all(root.join("site/two")).unwrap();
    std::fs::write(root.join("site/one/Note.md"), "one").unwrap();
    std::fs::write(root.join("site/two/Note.md"), "two").unwrap();
    std::fs::write(root.join("site/link.md"), "[note](Note)").unwrap();
    let (exit, _, stderr) = build(&root, &out, &[]);
    assert_eq!(exit, Exit::Content, "{stderr}");
    assert_eq!(tree(&out), deployed, "a failed build writes nothing");
}

#[test]
fn a_build_never_writes_or_removes_through_a_link() {
    let root = scratch("linked");
    let victim = root.join("victim");
    std::fs::create_dir_all(&victim).unwrap();
    let out = root.join("out");
    assert_eq!(build(&root, &out, &[]).0, Exit::Done);
    let index = std::fs::read(out.join("usage/index.html")).unwrap();
    std::fs::write(victim.join("index.html"), &index).unwrap();
    std::fs::remove_dir_all(out.join("usage")).unwrap();
    symlink(&victim, out.join("usage")).unwrap();
    let (exit, _, stderr) = build(&root, &out, &[]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
    assert!(stderr.contains("usage is a link"), "{stderr}");
    assert_eq!(std::fs::read(victim.join("index.html")).unwrap(), index, "the target is left alone");

    std::fs::remove_file(out.join("usage")).unwrap();
    std::fs::remove_file(out.join("404.html")).unwrap();
    symlink(victim.join("planted.html"), out.join("404.html")).unwrap();
    let (exit, _, stderr) = build(&root, &out, &[]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
    assert!(stderr.contains("404.html is a link"), "{stderr}");
    assert!(!victim.join("planted.html").exists(), "nothing is written through the link");
}

#[test]
fn a_build_refuses_a_file_in_the_way_that_no_build_wrote() {
    let root = scratch("occupied");
    let out = root.join("out");
    assert_eq!(build(&root, &out, &[]).0, Exit::Done);
    std::fs::remove_file(out.join("robots.txt")).unwrap();
    let manifest = std::fs::read_to_string(out.join(".ink-manifest.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let files = json["files"].as_array().unwrap();
    let kept: Vec<&serde_json::Value> = files.iter().filter(|file| file["path"] != "robots.txt").collect();
    std::fs::write(out.join(".ink-manifest.json"), serde_json::json!({ "files": kept }).to_string()).unwrap();
    std::fs::write(out.join("robots.txt"), "mine").unwrap();
    let (exit, _, stderr) = build(&root, &out, &[]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
    assert!(stderr.contains("robots.txt is in the way"), "{stderr}");
    assert_eq!(std::fs::read_to_string(out.join("robots.txt")).unwrap(), "mine");
    assert!(out.join("usage/index.html").is_file(), "the refusal came before any removal");
}
