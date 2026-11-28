//! The colour rules through the real binary on a pipe, where `auto` must mean no colour unless
//! the environment forces it: `NO_COLOR`, `CLICOLOR_FORCE`, `INK_COLOR`, `TERM` and `--color`,
//! on stderr, and never on stdout, which carries the result a script reads.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

use std::path::Path;
use std::process::Command;

/// What the environment and flag say, and whether stderr is coloured.
struct Case {
    env: &'static [(&'static str, &'static str)],
    flag: Option<&'static str>,
    coloured: bool,
}

const CASES: &[Case] = &[
    Case { env: &[], flag: None, coloured: false },
    Case { env: &[("CLICOLOR_FORCE", "1")], flag: None, coloured: true },
    Case { env: &[("CLICOLOR_FORCE", "0")], flag: None, coloured: false },
    Case { env: &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")], flag: None, coloured: false },
    Case { env: &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "")], flag: None, coloured: true },
    Case { env: &[("CLICOLOR_FORCE", "1"), ("CLICOLOR", "0")], flag: None, coloured: true },
    Case { env: &[("NO_COLOR", "1")], flag: Some("always"), coloured: true },
    Case { env: &[("CLICOLOR_FORCE", "1")], flag: Some("never"), coloured: false },
    Case { env: &[], flag: Some("always"), coloured: true },
    Case { env: &[("INK_COLOR", "always")], flag: None, coloured: true },
    Case { env: &[("INK_COLOR", "always")], flag: Some("never"), coloured: false },
    Case { env: &[("INK_COLOR", "never"), ("CLICOLOR_FORCE", "1")], flag: None, coloured: false },
    Case { env: &[("CLICOLOR_FORCE", "1"), ("TERM", "dumb")], flag: None, coloured: true },
];

#[test]
fn colour_follows_the_flag_then_the_environment_and_a_pipe_is_plain() {
    let site = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/readme");
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-colour");
    for (number, case) in CASES.iter().enumerate() {
        let out = root.join(number.to_string());
        std::fs::remove_dir_all(&out).ok();
        let mut command = Command::new(env!("CARGO_BIN_EXE_ink"));
        command.args(["build", "-C"]).arg(&site).arg("-o").arg(&out);
        if let Some(when) = case.flag {
            command.args(["--color", when]);
        }
        let ran = command.env_clear().envs(case.env.iter().copied()).output().unwrap();
        let (stdout, stderr) = (String::from_utf8_lossy(&ran.stdout), String::from_utf8_lossy(&ran.stderr));
        let shown = format!("env {:?} flag {:?}", case.env, case.flag);
        assert_eq!(ran.status.code(), Some(0), "{shown}: {stderr}");
        assert_eq!(stderr.contains('\u{1b}'), case.coloured, "{shown}: stderr was {stderr:?}");
        assert!(!stdout.contains('\u{1b}'), "{shown}: the result a script reads is never coloured: {stdout:?}");
        assert_eq!(stdout.trim().len(), 64, "{shown}: {stdout:?}");
    }
}

#[test]
fn json_is_never_coloured_whatever_the_environment_says() {
    let site = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/readme");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-colour/json");
    std::fs::remove_dir_all(&out).ok();
    let ran = Command::new(env!("CARGO_BIN_EXE_ink"))
        .args(["--json", "--color", "always", "build", "-C"])
        .arg(&site)
        .arg("-o")
        .arg(&out)
        .env_clear()
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&ran.stdout).into_owned() + &String::from_utf8_lossy(&ran.stderr);
    assert!(!text.contains('\u{1b}'), "{text:?}");
    let last = text.lines().find(|line| line.contains("uses.build/")).unwrap();
    serde_json::from_str::<serde_json::Value>(last).unwrap();
}
