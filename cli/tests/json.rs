//! The `--json` contract: every command prints exactly one JSON document on stdout, an error's
//! included, every line on stderr is an NDJSON event, and notices come only with `-v`.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_cli as cli;

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use cli::{Cli, Exit};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site").join(name)
}

/// A fresh scratch folder for `name`.
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-json").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// `ink <args>`: the exit, the one document on stdout, and the events on stderr. Fails unless
/// stdout is exactly one JSON document and every stderr line is one.
fn ink(args: &[&OsStr]) -> (Exit, serde_json::Value, Vec<serde_json::Value>) {
    let args = std::iter::once(OsStr::new("ink")).chain(args.iter().copied());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = Cli::start(args, &mut out, &mut err).unwrap();
    let stdout = String::from_utf8(out).unwrap();
    let stderr = String::from_utf8(err).unwrap();
    assert_eq!(stdout.lines().count(), 1, "one document on stdout: {stdout}");
    let document = serde_json::from_str(&stdout).unwrap_or_else(|error| panic!("{error}: {stdout}"));
    let events =
        stderr.lines().map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"))).collect();
    (exit, document, events)
}

fn os(text: &str) -> &OsStr {
    OsStr::new(text)
}

#[test]
fn a_build_prints_its_document_written_or_not() {
    let out = scratch("built");
    let (exit, built, _) = ink(&[os("build"), fixture("readme").as_os_str(), os("-o"), out.as_os_str(), os("--json")]);
    assert_eq!(exit, Exit::Done);
    assert_eq!(built["schema"], "uses.build/v1");
    assert_eq!(built["written"], true);
    assert_eq!(built["deploy"].as_str().unwrap().len(), 64);

    let failed = scratch("failed");
    let (exit, built, events) =
        ink(&[os("build"), fixture("ambiguous").as_os_str(), os("-o"), failed.as_os_str(), os("--json")]);
    assert_eq!(exit, Exit::Content);
    assert_eq!(built["schema"], "uses.build/v1");
    assert_eq!(built["written"], false);
    assert_eq!(built["errors"], 1);
    assert_eq!(events[0]["code"], "link.target.ambiguous");
}

#[test]
fn an_error_is_one_document_on_stdout_and_nothing_on_stderr() {
    let out = scratch("foreign");
    std::fs::write(out.join("keep.txt"), "mine").unwrap();
    let (exit, failed, events) =
        ink(&[os("build"), fixture("readme").as_os_str(), os("-o"), out.as_os_str(), os("--json")]);
    assert_eq!(exit, Exit::Usage);
    assert_eq!(failed["schema"], "uses.error/v1");
    assert_eq!(failed["exit"], 2);
    assert_eq!(failed["error"], "output.folder.foreign");
    assert!(failed["what"].as_str().unwrap().contains("holds files no `ink build` wrote"), "{failed}");
    assert!(events.is_empty(), "{events:?}");

    let missing = scratch("missing").join("never");
    let (exit, failed, events) = ink(&[os("check"), missing.as_os_str(), os("--json")]);
    assert_eq!(exit, Exit::Usage);
    assert_eq!(failed["error"], "source.root.unreadable");
    assert_eq!(failed["diagnostic"]["code"], "source.root.unreadable");
    assert!(!failed["causes"].as_array().unwrap().is_empty(), "{failed}");
    assert!(events.is_empty(), "{events:?}");

    let (exit, failed, _) =
        ink(&[os("explain"), os("-C"), fixture("readme").as_os_str(), os("nosuch.md"), os("--json")]);
    assert_eq!(exit, Exit::Usage);
    assert_eq!(failed["error"], "explain.path.unknown");

    let (exit, failed, _) = ink(&[os("--json"), os("build")]);
    assert_eq!(exit, Exit::Usage);
    assert_eq!(failed["error"], "cli.usage");
}

#[test]
fn a_path_that_is_not_utf8_is_written_lossily() {
    let root = scratch("lossy");
    let out: OsString = [root.as_os_str(), OsStr::from_bytes(b"/out-\xff")].into_iter().collect();
    let (exit, built, _) = ink(&[os("build"), fixture("readme").as_os_str(), os("-o"), &out, os("--json")]);
    assert_eq!(exit, Exit::Done, "{built}");
    assert!(built["out"].as_str().unwrap().ends_with("/out-\u{fffd}"), "{built}");
}

#[test]
fn notices_come_only_with_verbose() {
    let is_notice = |event: &serde_json::Value| event["severity"] == "notice";
    let (_, checked, events) = ink(&[os("check"), fixture("notes").as_os_str(), os("--json")]);
    assert_eq!(checked["schema"], "uses.check/v1");
    assert!(!events.iter().any(is_notice), "{events:?}");
    let (_, _, events) = ink(&[os("check"), fixture("notes").as_os_str(), os("--json"), os("-v")]);
    assert!(events.iter().any(is_notice), "{events:?}");
}
