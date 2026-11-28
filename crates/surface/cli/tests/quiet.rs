//! How much `ink` says: `-q` drops stage lines and keeps warnings, the result and what a build
//! makes public, `-qq` keeps only errors, and an error is never silenced.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

use std::path::Path;
use std::process::Command;

/// What one run said.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

fn build(fixture: &str, name: &str, flags: &[&str]) -> Ran {
    let site = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site").join(fixture);
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-quiet").join(name);
    std::fs::remove_dir_all(&out).ok();
    let ran = Command::new(env!("CARGO_BIN_EXE_ink"))
        .args(flags)
        .arg("build")
        .arg("-C")
        .arg(&site)
        .arg("-o")
        .arg(&out)
        .env_clear()
        .output()
        .unwrap();
    Ran {
        code: ran.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&ran.stdout).into_owned(),
        err: String::from_utf8_lossy(&ran.stderr).into_owned(),
    }
}

#[test]
fn quiet_drops_the_stages_and_double_quiet_leaves_only_errors() {
    let plain = build("broken-frontmatter", "plain", &[]);
    assert_eq!(plain.code, 0, "{}", plain.err);
    assert!(plain.err.contains("Rendered") && plain.err.contains("Built"), "the stage lines: {}", plain.err);

    let quiet = build("broken-frontmatter", "quiet", &["-q"]);
    assert_eq!(quiet.code, 0, "{}", quiet.err);
    assert!(!quiet.err.contains("Rendered") && !quiet.err.contains("Built"), "-q drops the stages: {}", quiet.err);
    assert!(quiet.err.contains("frontmatter.syntax.invalid"), "-q keeps warnings: {}", quiet.err);
    assert!(quiet.err.contains("newly public"), "-q keeps what the build makes public: {}", quiet.err);
    assert_eq!(quiet.out.trim().len(), 64, "the result is still printed: {}", quiet.out);

    let silent = build("broken-frontmatter", "silent", &["-qq"]);
    assert_eq!(silent.code, 0, "{}", silent.err);
    assert!(silent.err.trim().is_empty(), "-qq says nothing but errors: {}", silent.err);
    assert_eq!(silent.out, plain.out, "the deploy id is the same however loud the run was");

    let failed = build("ambiguous", "failed", &["-qq"]);
    assert_eq!(failed.code, 1, "{}", failed.err);
    assert!(failed.err.contains("link.target.ambiguous"), "an error is never silenced: {}", failed.err);
}
