//! The command tree seen from outside: help, completions, the manual, the exit table, the codes,
//! the new argument shapes and the verbs that are not built yet.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

use std::ffi::OsString;

use uses_ink_cli::{Cli, Exit};

/// Runs `ink <args>`: the exit, stdout and stderr.
fn ink(args: &[&str]) -> (Exit, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args: Vec<OsString> = std::iter::once("ink").chain(args.iter().copied()).map(Into::into).collect();
    let exit = Cli::start(args, &mut out, &mut err).unwrap();
    (exit, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn the_root_help_lists_the_groups_with_their_aliases() {
    let (exit, help, _) = ink(&["--help"]);
    assert_eq!(exit, Exit::Done);
    for group in ["\nWrite\n", "\nUnderstand\n", "\nPublish\n", "\nAccount\n", "\nOperate\n", "\nTools\n"] {
        assert!(help.contains(group), "{group}: {help}");
    }
    for line in [
        "  check, c  ",
        "  build, b  ",
        "  deploy  ",
        "\nGlobal options\n",
        "-C, --dir <DIR>",
        "\nExamples\n",
        "\nEnvironment\n",
    ] {
        assert!(help.contains(line), "{line}: {help}");
    }
    assert!(!help.contains("  deploys "), "the hidden alias is not listed: {help}");
    assert!(!help.contains('\u{1b}'), "plain without colour: {help}");
}

#[test]
fn a_command_without_a_subcommand_prints_its_help_on_stderr_and_is_a_usage_error() {
    let (exit, out, err) = ink(&[]);
    assert_eq!((exit, out.as_str()), (Exit::Usage, ""));
    assert!(err.contains("Usage: ink [OPTIONS] <COMMAND>"), "{err}");
}

#[test]
fn a_leaf_has_option_groups_examples_and_exit_codes_and_short_help_stops_early() {
    let (_, long, _) = ink(&["build", "--help"]);
    for line in [
        "Usage: ink build [OPTIONS]",
        "\nOutput\n",
        "\nSource\n",
        "\nWork\n",
        "\nVerification\n",
        "-o, --out <DIR>",
        "\nExamples\n",
        "\nExit codes\n",
        "  6  ",
    ] {
        assert!(long.contains(line), "{line}: {long}");
    }
    let (_, short, _) = ink(&["build", "-h"]);
    assert!(short.contains("-o, --out <DIR>") && !short.contains("\nExamples\n"), "{short}");
}

#[test]
fn an_alias_gets_the_help_of_its_command_and_a_group_lists_its_commands() {
    let (_, help, _) = ink(&["b", "--help"]);
    assert!(help.contains("Usage: ink build"), "{help}");
    let (_, group, _) = ink(&["deploy", "--help"]);
    for word in ["rollback", "resume", "show", "list"] {
        assert!(group.contains(&format!("  {word}")), "{word}: {group}");
    }
}

#[test]
fn help_exit_prints_the_closed_set_and_the_two_that_are_not_in_it() {
    let (exit, table, _) = ink(&["help", "exit"]);
    assert_eq!(exit, Exit::Done);
    for code in ["  0  ", "  1  ", "  2  ", "  3  ", "  4  ", "  5  ", "  6  "] {
        assert!(table.contains(code), "{code}: {table}");
    }
    assert!(table.contains("130") && table.contains("101"), "{table}");
}

#[test]
fn help_names_a_command_a_code_or_nothing() {
    let (exit, help, _) = ink(&["help", "deploy", "rollback"]);
    assert_eq!(exit, Exit::Done);
    assert!(help.contains("Usage: ink deploy rollback"), "{help}");
    let (_, said, _) = ink(&["help", "link.target.missing"]);
    assert!(said.contains("points at no file"), "{said}");
    let (_, decision, _) = ink(&["help", "page.title"]);
    assert!(decision.contains("decided for each file"), "{decision}");
    let (exit, _, err) = ink(&["help", "no.such.thing"]);
    assert_eq!(exit, Exit::Usage);
    assert!(err.contains("no command, no topic"), "{err}");
}

#[test]
fn every_shell_gets_a_script_that_knows_the_deploy_verbs() {
    for shell in ["bash", "zsh", "fish", "nushell", "elvish", "powershell"] {
        let (exit, script, _) = ink(&["completions", shell]);
        assert_eq!(exit, Exit::Done, "{shell}");
        assert!(script.contains("rollback") && script.contains("no-progress"), "{shell}");
    }
}

#[test]
fn the_manual_is_roff_with_the_sections_clap_has_no_place_for() {
    let (exit, page, _) = ink(&["man"]);
    assert_eq!(exit, Exit::Done);
    assert!(page.starts_with(".ie") || page.starts_with(".TH"), "{}", &page[..40]);
    for section in ["EXAMPLES", "EXIT STATUS", "ENVIRONMENT"] {
        assert!(page.contains(section), "{section}");
    }
}

#[test]
fn a_folder_verb_with_no_link_is_a_usage_error_that_says_how_to_link() {
    let empty = tempfile::tempdir().unwrap();
    let dir = empty.path().to_str().unwrap();
    for verb in ["push", "status", "watch", "export"] {
        let (exit, out, err) = ink(&["-C", dir, verb, "--api", "http://127.0.0.1:1"]);
        assert_eq!((exit, out.as_str()), (Exit::Usage, ""), "{verb}: {err}");
        assert!(err.contains("link.file.missing") && err.contains("ink init"), "{verb}: {err}");
    }
    let (_, json, _) = ink(&["-C", dir, "--json", "push", "--api", "http://127.0.0.1:1"]);
    assert!(json.contains("link.file.missing"), "{json}");
    let (exit, _, err) = ink(&["-C", dir, "init", "--api", "http://127.0.0.1:1"]);
    assert_eq!(exit, Exit::Auth, "no key comes before no handle: {err}");
}

#[test]
fn the_deploy_verbs_without_a_key_are_a_sign_in_error() {
    let site = "0123456789abcdef0123456789abcdef";
    let (exit, _, err) = ink(&["deploy", "list", "--api", "http://127.0.0.1:1", "--site", site]);
    assert_eq!(exit, Exit::Auth, "{err}");
    assert!(err.contains("INK_KEY") && err.contains("ink login"), "{err}");
    let (exit, _, _) = ink(&["deploys", "--site", site]);
    assert_eq!(exit, Exit::Usage);
}

#[test]
fn the_global_options_are_accepted_before_and_after_the_command() {
    let (exit, _, _) = ink(&["-q", "--color", "never", "--no-progress", "help"]);
    assert_eq!(exit, Exit::Done);
    let (exit, _, _) = ink(&["help", "-q", "--color=never", "--no-progress"]);
    assert_eq!(exit, Exit::Done);
}
