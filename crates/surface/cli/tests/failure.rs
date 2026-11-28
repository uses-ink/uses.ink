//! Every error a user can reach without a network, through the real binary: each one has a
//! code, a fix and what it left behind, exits as `ink help exit` says, and says the same in
//! `--json` as one document on stdout.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

use std::path::Path;
use std::process::Command;

/// One way to fail: the arguments, the variables, how it ends and what it is called.
struct Row {
    args: &'static [&'static str],
    env: &'static [(&'static str, &'static str)],
    exit: i32,
    code: &'static str,
}

/// What a run said.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

fn site(name: &str) -> String {
    format!("{}/fixture/site/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// `ink <args>` in a scratch folder with nothing in the environment but `env`.
fn ink(args: &[&str], env: &[(&str, &str)]) -> Ran {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-failure");
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ink"))
        .args(args)
        .current_dir(&dir)
        .env_clear()
        .envs(env.iter().copied())
        .output()
        .unwrap();
    Ran {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// What the user can type wrong and what `ink` answers. `@name` in an argument is the fixture
/// site of that name.
#[allow(clippy::too_many_lines, reason = "a table of rows, one line each")]
fn rows() -> Vec<Row> {
    let down: &[(&str, &str)] = &[("INK_API", "http://127.0.0.1:1")];
    vec![
        Row { args: &["build", "-C", "/nonexistent"], env: &[], exit: 2, code: "source.root.unreadable" },
        Row { args: &["check", "-C", "@ambiguous"], env: &[], exit: 1, code: "link.target.ambiguous" },
        Row { args: &["explain", "nope.md", "-C", "@blog"], env: &[], exit: 2, code: "explain.path.unknown" },
        Row { args: &["explain", "--why", "bogus", "-C", "@blog"], env: &[], exit: 2, code: "decision.code.unknown" },
        Row { args: &["pin", "a", "b", "-C", "@blog"], env: &[], exit: 2, code: "decision.code.unknown" },
        Row { args: &["select", "missing.md", "-C", "@vault"], env: &[], exit: 2, code: "explain.path.unknown" },
        Row { args: &["push"], env: &[], exit: 2, code: "link.file.missing" },
        Row { args: &["export"], env: &[], exit: 2, code: "link.file.missing" },
        Row { args: &["status"], env: down, exit: 2, code: "link.file.missing" },
        Row { args: &["init"], env: &[], exit: 2, code: "deploy.setting.missing" },
        Row { args: &["whoami"], env: &[], exit: 2, code: "deploy.setting.missing" },
        Row { args: &["deploy", "rollback", "zz"], env: &[], exit: 2, code: "deploy.setting.missing" },
        Row {
            args: &["deploy", "show", "zz", "--site", "00000000000000000000000000000000"],
            env: down,
            exit: 3,
            code: "deploy.setting.missing",
        },
        Row { args: &["key", "add", "--site", "zz"], env: down, exit: 4, code: "deploy.api.unreachable" },
        Row { args: &["login", "--api", "http://127.0.0.1:1"], env: &[], exit: 2, code: "auth.store.failed" },
        Row {
            args: &["login", "--data", "/nonexistent", "--api", "http://127.0.0.1:1"],
            env: &[("HOME", "/nonexistent")],
            exit: 2,
            code: "auth.operator.unreadable",
        },
        Row { args: &["serve", "/nonexistent"], env: &[], exit: 2, code: "serve.folder.unservable" },
        Row {
            args: &["build", "-C", "@blog", "--rev", "x", "-o", "unused"],
            env: &[],
            exit: 2,
            code: "source.rev.unversioned",
        },
        Row { args: &["help", "bogus"], env: &[], exit: 2, code: "help.topic.unknown" },
        Row { args: &["bogus"], env: &[], exit: 2, code: "cli.usage" },
        Row { args: &["build", "--bogus"], env: &[], exit: 2, code: "cli.usage" },
        Row { args: &["preview", "--port", "abc"], env: &[], exit: 2, code: "cli.usage" },
        Row { args: &["--color", "bogus", "build"], env: &[], exit: 2, code: "cli.usage" },
        Row { args: &["completions", "bogus"], env: &[], exit: 2, code: "cli.usage" },
    ]
}

/// `args` with each `@name` made the path of its fixture site.
fn sited(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.strip_prefix('@').map_or_else(|| (*arg).to_owned(), site)).collect()
}

#[test]
fn every_error_a_user_reaches_has_a_code_a_fix_and_a_state() {
    for row in rows() {
        let args = sited(row.args);
        let shown = args.join(" ");
        let ran = ink(&args.iter().map(String::as_str).collect::<Vec<_>>(), row.env);
        assert_eq!(ran.code, row.exit, "ink {shown}: {}{}", ran.out, ran.err);
        let head = format!("error[{}]", row.code);
        assert!(ran.err.contains(&head), "ink {shown} names its code `{}`: {}", row.code, ran.err);
        assert!(
            ran.err.lines().any(|line| line.trim_start().starts_with("fix: ")),
            "ink {shown} says what to do: {}",
            ran.err
        );
        // a refused folder is reported as its diagnostic, which has no state line of its own
        if !matches!(row.code, "link.target.ambiguous" | "source.root.unreadable") {
            assert!(ran.err.contains("nothing was changed"), "ink {shown} says what it left: {}", ran.err);
        }
        assert!(ran.out.trim().is_empty(), "ink {shown} printed a result with an error: {}", ran.out);
        assert!(!ran.err.contains("\u{1b}["), "ink {shown} coloured a pipe: {:?}", ran.err);
    }
}

#[test]
fn an_error_in_json_is_one_document_with_its_code_fix_and_state() {
    type Case = (&'static [&'static str], &'static [(&'static str, &'static str)], &'static str);
    let documented: &[Case] = &[
        (&["push"], &[], "link.file.missing"),
        (&["whoami"], &[], "deploy.setting.missing"),
        (&["key", "add"], &[("INK_API", "http://127.0.0.1:1")], "deploy.api.unreachable"),
        (&["serve", "/nonexistent"], &[], "serve.folder.unservable"),
    ];
    for (args, env, code) in documented {
        let mut all = vec!["--json"];
        all.extend(*args);
        let ran = ink(&all, env);
        assert!(ran.err.trim().is_empty(), "ink {}: json leaves stderr empty: {}", args.join(" "), ran.err);
        let document: serde_json::Value = serde_json::from_str(ran.out.lines().last().unwrap()).unwrap();
        assert_eq!(document["schema"], "uses.error/v1", "{document}");
        assert_eq!(document["error"], *code, "{document}");
        assert!(document["fix"].as_str().is_some_and(|fix| !fix.is_empty()), "a fix in {document}");
        assert!(document["what"].as_str().is_some_and(|what| !what.is_empty()), "{document}");
        assert_eq!(document["state"]["kind"], "unchanged", "{document}");
        assert_eq!(i64::from(ran.code), document["exit"].as_i64().unwrap(), "{document}");
    }
}
