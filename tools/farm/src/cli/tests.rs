//! The command line: flags, verbs and their errors.

use super::*;

fn parse(line: &str) -> Result<Command, Usage> {
    Command::parse(line.split_whitespace().map(str::to_owned).collect())
}

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

#[test]
fn a_double_dash_after_the_subcommand_and_what_follows_reach_cargo_verbatim() {
    let line = "clippy -p x --all-targets -- -D warnings";
    assert_eq!(parse(&format!("cargo {line}")).unwrap().verb, Verb::Cargo(words(line)));
    let more = "nextest run -p x -- --nocapture --json";
    assert_eq!(parse(&format!("--detach cargo {more}")).unwrap().verb, Verb::Cargo(words(more)));
    let flags = parse("cargo clippy -- --json -D warnings").unwrap();
    assert!(!flags.flags.json, "after the subcommand `--json` is cargo's, `--` or not");
    assert_eq!(parse("bless -p x -- name").unwrap().verb, Verb::Bless(words("-p x -- name")));
}

#[test]
fn a_check_names_jobs_or_none() {
    assert_eq!(parse("check").unwrap().verb, Verb::Check(vec![]));
    assert_eq!(
        parse("check lint test").unwrap().verb,
        Verb::Check(vec!["lint".parse().unwrap(), "test".parse().unwrap()])
    );
    assert!(matches!(parse("check ../x"), Err(Usage::Task(_))));
}

#[test]
fn cargo_arguments_pass_through_and_farm_flags_stop_at_the_first_other_word() {
    let command = parse("--json cargo --detach clippy -p engine --verbose").unwrap();
    assert!(command.flags.json && command.flags.detach);
    assert!(!command.flags.verbose, "after the cargo arguments start, `--verbose` is cargo's");
    assert_eq!(command.verb, Verb::Cargo(["clippy", "-p", "engine", "--verbose"].map(str::to_owned).to_vec()));
    let command = parse("cargo -- --json nextest").unwrap();
    assert!(!command.flags.json);
    assert_eq!(command.verb, Verb::Cargo(["--json", "nextest"].map(str::to_owned).to_vec()));
    let command = parse("cargo -- clippy -p x -- -D warnings").unwrap();
    assert_eq!(command.verb, Verb::Cargo(words("clippy -p x -- -D warnings")), "a leading `--` is the farm's");
    assert!(matches!(parse("cargo"), Err(Usage::Missing { .. })));
    assert!(matches!(parse("cargo --json"), Err(Usage::Missing { .. })));
}

#[test]
fn fresh_and_budget_are_farm_flags_before_or_after_the_verb_but_not_after_cargo_words() {
    let flags = parse("--fresh check --budget 3 lint").unwrap().flags;
    assert!(flags.fresh);
    assert_eq!(flags.budget, Some(3));
    assert!(parse("cargo --fresh nextest run").unwrap().flags.fresh);
    assert!(!parse("cargo nextest run --fresh").unwrap().flags.fresh, "after cargo's words it is cargo's");
    assert!(parse("check --budget x").is_err());
    assert!(!parse("check").unwrap().flags.fresh);
}

#[test]
fn fmt_takes_nothing() {
    assert_eq!(parse("fmt").unwrap().verb, Verb::Fmt);
    assert!(matches!(parse("fmt -p cli"), Err(Usage::Extra { .. })));
}

#[test]
fn bless_takes_nextest_arguments_or_none() {
    assert_eq!(parse("bless").unwrap().verb, Verb::Bless(vec![]));
    assert_eq!(parse("bless -p cli").unwrap().verb, Verb::Bless(vec!["-p".to_owned(), "cli".to_owned()]));
}

#[test]
fn wait_and_log_take_a_job_id_and_their_flags() {
    let command = parse("wait petal.1a2 --timeout 30").unwrap();
    assert_eq!(command.flags.timeout, Seconds(30));
    assert!(matches!(command.verb, Verb::Wait(_)));
    assert_eq!(parse("log petal.1a2 --tail 20").unwrap().flags.tail, Some(20));
    assert!(matches!(parse("wait"), Err(Usage::Missing { .. })));
    assert!(matches!(parse("wait nope"), Err(Usage::Job(_))));
    assert!(matches!(parse("wait a.1 b.2"), Err(Usage::Extra { .. })));
}

#[test]
fn workspace_makes_or_forgets() {
    assert!(matches!(parse("workspace x --at main").unwrap().verb, Verb::Make(_)));
    assert!(matches!(parse("workspace --forget x").unwrap().verb, Verb::Forget(_)));
    assert!(matches!(parse("workspace ../x"), Err(Usage::Name(_))));
    assert!(matches!(parse("workspace x --at --evil"), Err(Usage::Flag { .. })));
}

#[test]
fn defaults_are_the_documented_ones_and_bad_input_is_usage() {
    let flags = parse("status").unwrap().flags;
    assert_eq!((flags.timeout, flags.limit), (Seconds::TIMEOUT, None));
    for bad in ["", "frob", "status --timeout", "status --timeout x", "status --host A b", "status extra"] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn test_passes_its_arguments_to_nextest_and_affected_and_base_have_their_verbs() {
    assert_eq!(parse("test").unwrap().verb, Verb::Test(vec![]));
    let command = parse("--detach test -E test(a) --no-fail-fast").unwrap();
    assert_eq!(command.verb, Verb::Test(words("-E test(a) --no-fail-fast")));
    assert!(command.flags.detach);
    assert_eq!(parse("test --base main~2 -p x").unwrap().flags.base.unwrap().as_str(), "main~2");
    let command = parse("check test --affected --base main").unwrap();
    assert!(command.flags.affected);
    assert_eq!(command.verb, Verb::Check(vec!["test".parse().unwrap()]));
    assert!(parse("cargo check --affected").is_ok(), "after the cargo words it is cargo's");
    assert!(matches!(parse("--affected cargo check"), Err(Usage::Flag { .. })));
    assert!(matches!(parse("--affected test"), Err(Usage::Flag { .. })), "test is always affected");
    assert!(matches!(parse("--base main status"), Err(Usage::Flag { .. })));
    assert!(matches!(parse("--base -x test"), Err(Usage::Flag { .. })));
}
