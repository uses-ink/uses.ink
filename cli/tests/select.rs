//! `ink select`: the listing of every site compared with `fixture/select/<name>.snap`, and a
//! selection recorded, taken back, planned whole and refused (decisions 0042, open.md Q16 and
//! Q17). A vault's new note is private until selected; a public source publishes by default and
//! selecting only states it.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_cli as cli;

use std::path::{Path, PathBuf};

use clap::Parser;
use cli::{Cli, Exit};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site")
}

/// A fresh copy of the fixture site `name`.
fn copied(name: &str, test: &str) -> PathBuf {
    let to = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-select").join(test);
    if to.exists() {
        std::fs::remove_dir_all(&to).unwrap();
    }
    copy(&fixtures().join(name), &to);
    to
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() { copy(&path, &target) } else { std::fs::copy(&path, &target).map(|_| ()).unwrap() }
    }
}

/// `ink <args>`: the exit, stdout and stderr.
fn ink(args: &[&str]) -> (Exit, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("ink").chain(args.iter().copied())).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = cli.run(&mut out, &mut err);
    (exit, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn select(dir: &Path, args: &[&str]) -> (Exit, String, String) {
    let mut all = vec!["select", "-C", dir.to_str().unwrap()];
    all.extend(args);
    ink(&all)
}

/// The listing's line for `path`.
fn row<'a>(listing: &'a str, path: &str) -> &'a str {
    listing.lines().find(|line| line.trim_start().starts_with(path)).unwrap_or_else(|| panic!("{path} in {listing}"))
}

/// The names of the sites, sorted.
fn sites() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_listing_of_every_site_is_as_expected() {
    let cases = sites().into_iter().map(|name| {
        let (_, listing, _) = select(&fixtures().join(&name), &[]);
        (name, listing)
    });
    fixture::cases("fixture/select", cases);
}

#[test]
fn a_new_note_in_a_vault_is_private_until_it_is_selected() {
    let dir = copied("vault", "vault-new-note");
    std::fs::write(dir.join("Notes/New.md"), "---\ntitle: New\n---\nHello.\n").unwrap();
    let (exit, listing, stderr) = select(&dir, &[]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(listing.starts_with("selection: explicit"), "{listing}");
    assert!(row(&listing, "Notes/New.md").contains("not selected"), "{listing}");
    assert!(row(&listing, "Notes/Rust.md").contains("the frontmatter of Notes/Rust.md says so"), "{listing}");

    let (exit, said, stderr) = select(&dir, &["Notes/New.md"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(said.contains("added `publish: true` to the frontmatter"), "{said}");
    assert!(said.ends_with("wrote 1 file\n"), "{said}");
    let text = std::fs::read_to_string(dir.join("Notes/New.md")).unwrap();
    assert_eq!(text, "---\ntitle: New\npublish: true\n---\nHello.\n");

    let (exit, listing, stderr) = select(&dir, &[]);
    assert_eq!(exit, Exit::Done, "the guard is quiet: {stderr}");
    assert!(row(&listing, "Notes/New.md").contains("the frontmatter of Notes/New.md says so"), "{listing}");
    let published = listing.split("not published").next().unwrap();
    assert!(published.contains("Notes/New.md"), "{listing}");
}

#[test]
fn a_public_source_keeps_publishing_by_default_and_selecting_only_states_it() {
    let dir = copied("readme", "public");
    let (exit, listing, _) = select(&dir, &[]);
    assert_eq!(exit, Exit::Done);
    assert!(listing.starts_with("selection: implicit"), "{listing}");
    assert!(row(&listing, "usage.md").contains("by default: a public source publishes its pages"), "{listing}");
    assert!(listing.contains("not published (0)"), "{listing}");

    std::fs::write(dir.join("usage.md"), "---\ntitle: Usage\n---\n# Usage\n\nRun it.\n").unwrap();
    let (exit, _, stderr) = select(&dir, &["usage.md"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    let (_, listing, _) = select(&dir, &[]);
    assert!(row(&listing, "usage.md").contains("the frontmatter of usage.md says so"), "{listing}");
    assert!(listing.contains("not published (0)"), "{listing}");
}

#[test]
fn a_folder_is_an_include_entry_and_comments_stay() {
    let dir = copied("vault-markdown", "folder");
    std::fs::write(dir.join("ink.toml"), "# my vault\n[site]\nformat = \"ink:format-markdown\" # plain\n").unwrap();
    std::fs::create_dir_all(dir.join("Notes")).unwrap();
    std::fs::write(dir.join("Notes/a.md"), "# A\n").unwrap();
    let (exit, said, stderr) = select(&dir, &["--folder", "Notes/**"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(said.contains("include Notes/**"), "{said}");
    assert!(said.contains("added to `include` in ink.toml, matches 1 file"), "{said}");
    let text = std::fs::read_to_string(dir.join("ink.toml")).unwrap();
    assert_eq!(text, "include = [\"Notes/**\"]\n# my vault\n[site]\nformat = \"ink:format-markdown\" # plain\n");

    let (_, listing, _) = select(&dir, &[]);
    assert!(row(&listing, "Notes/a.md").contains("selected by the `include` pattern `Notes/**` in ink.toml"), "{listing}");

    let (_, again, _) = select(&dir, &["--folder", "Notes/**"]);
    assert!(again.contains("`include` in ink.toml already lists it"), "{again}");
    assert!(again.ends_with("nothing to write\n"), "{again}");

    let (exit, gone, stderr) = select(&dir, &["--folder", "Notes/**", "--remove"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(gone.contains("removed from `include` in ink.toml"), "{gone}");
    let (_, missing, _) = select(&dir, &["--folder", "Notes/**", "--remove"]);
    assert!(missing.contains("`include` in ink.toml does not list it"), "{missing}");
}

#[test]
fn remove_takes_publish_true_out_and_leaves_a_false() {
    let dir = copied("vault", "remove");
    let rust = dir.join("Notes/Rust.md");
    let before = std::fs::read_to_string(&rust).unwrap();
    let (exit, said, stderr) = select(&dir, &["Notes/Rust.md", "--remove"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(said.contains("removed `publish: true` from the frontmatter"), "{said}");
    assert_eq!(std::fs::read_to_string(&rust).unwrap(), before.replacen("publish: true\n", "", 1));
    let (_, again, _) = select(&dir, &["Notes/Rust.md", "--remove"]);
    assert!(again.contains("the frontmatter has no `publish: true` to remove"), "{again}");

    std::fs::write(&rust, "---\npublish: false\n---\n# Rust\n").unwrap();
    let (_, kept, _) = select(&dir, &["Notes/Rust.md", "--remove"]);
    assert!(kept.contains("`publish: false`, which is no selection: left as it is"), "{kept}");
    assert_eq!(std::fs::read_to_string(&rust).unwrap(), "---\npublish: false\n---\n# Rust\n");
}

#[test]
fn a_dry_run_says_what_it_would_write_and_writes_nothing() {
    let dir = copied("vault", "dry");
    let private = dir.join("Private.md");
    let before = std::fs::read_to_string(&private).unwrap();
    let (exit, said, stderr) = select(&dir, &["Private.md", "--dry-run"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(said.contains("added `publish: true` to the frontmatter"), "{said}");
    assert!(said.ends_with("dry run: nothing written\n"), "{said}");
    assert_eq!(std::fs::read_to_string(&private).unwrap(), before);
    let (_, said, _) = select(&dir, &["--folder", "Notes/**", "--dry-run"]);
    assert!(said.ends_with("dry run: nothing written\n"), "{said}");
    assert!(!dir.join("ink.toml").exists());
}

#[test]
fn nothing_is_written_when_one_file_is_refused() {
    let dir = copied("vault", "whole");
    let private = dir.join("Private.md");
    let before = std::fs::read_to_string(&private).unwrap();
    std::fs::write(dir.join("Notes/Bare.md"), "No frontmatter.\n").unwrap();
    let (exit, said, stderr) = select(&dir, &["Private.md", "Notes/Bare.md"]);
    assert_eq!(exit, Exit::Usage);
    assert_eq!(said, "");
    assert!(stderr.contains("`publish` cannot be written into the frontmatter of Notes/Bare.md"), "{stderr}");
    assert!(stderr.contains("caused by: the file has no frontmatter block"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&private).unwrap(), before, "the file before it was not written either");
}

#[test]
fn a_file_that_cannot_be_selected_is_a_typed_refusal() {
    let dir = copied("vault", "refused");
    let refused = |path: &str| {
        let (exit, _, stderr) = select(&dir, &[path]);
        assert_eq!(exit, Exit::Usage);
        stderr
    };
    assert!(refused("Archive/ferris.png").contains("is a plain file"), "plain");
    assert!(refused(".obsidian/app.json").contains("it is ignored"), "ignored");
    assert!(refused("Nowhere.md").contains("is not a file of the folder"), "unknown");
    let (exit, _, stderr) = select(&dir, &["--folder", "a//b"]);
    assert_eq!(exit, Exit::Usage);
    assert!(stderr.contains("`a//b` is not a glob pattern"), "{stderr}");

    std::fs::write(dir.join("Notes/Broken.md"), "---\ntitle: [oops\n---\n").unwrap();
    assert!(refused("Notes/Broken.md").contains("the frontmatter cannot be read"), "invalid");
    std::fs::write(dir.join("ink.toml"), "include = \"notes\"\n").unwrap();
    let (exit, _, stderr) = select(&dir, &["--folder", "Notes/**"]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
    assert!(stderr.contains("the selection could not be written into ink.toml"), "{stderr}");
    assert!(stderr.contains("caused by: `include` in ink.toml is not an array of strings"), "{stderr}");
}

#[test]
fn a_pin_of_publish_outranks_the_frontmatter_and_is_said() {
    let dir = copied("vault", "pinned");
    std::fs::write(dir.join("ink.toml"), "[page.\"Private.md\"]\npublish = false\n").unwrap();
    let (exit, said, stderr) = select(&dir, &["Private.md"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(said.contains("`page.publish` is pinned in ink.toml, which outranks the frontmatter"), "{said}");
    assert!(said.ends_with("nothing to write\n"), "{said}");
}

#[test]
fn json_is_one_document_with_the_pages_and_the_steps() {
    let dir = copied("vault", "json");
    let (exit, listing, _) = select(&dir, &["--json"]);
    assert_eq!(exit, Exit::Done);
    let listing: serde_json::Value = serde_json::from_str(&listing).unwrap();
    assert_eq!(listing["schema"], "uses.select/v1");
    assert_eq!(listing["selection"], "explicit");
    assert_eq!(listing["written"], 0);
    let pages = listing["pages"].as_array().unwrap();
    let private = pages.iter().find(|page| page["subject"]["file"] == "Private.md").unwrap();
    assert_eq!((&private["code"], &private["value"]), (&"page.publish".into(), &false.into()));
    assert_eq!(private["evidence"]["default"]["reason"], "not-selected");

    let (_, written, _) = select(&dir, &["Private.md", "--json"]);
    let written: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(written["written"], 1);
    let step = &written["steps"][0];
    assert_eq!((step["place"].as_str(), step["path"].as_str(), step["outcome"].as_str()), (Some("file"), Some("Private.md"), Some("added")));

    let (_, folder, _) = select(&dir, &["--folder", "Notes/**", "--json", "--dry-run"]);
    let folder: serde_json::Value = serde_json::from_str(&folder).unwrap();
    assert_eq!(folder["written"], 0);
    let step = &folder["steps"][0];
    assert_eq!((step["place"].as_str(), step["pattern"].as_str(), step["outcome"].as_str()), (Some("folder"), Some("Notes/**"), Some("listed")));
    assert_eq!(step["matches"], 2);
}
