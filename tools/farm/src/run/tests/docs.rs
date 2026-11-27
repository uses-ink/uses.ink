//! `docs/workflow.md` names every verb, flag, exit code and config key.

use super::*;

#[test]
fn the_workflow_doc_names_every_verb_flag_exit_code_and_config_key() {
    let doc = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/workflow.md"))
        .unwrap();
    for action in Action::ALL {
        assert!(doc.contains(&format!("`farm {action}")), "verb {action}");
    }
    for exit in Exit::ALL {
        assert!(doc.contains(&format!("| {} | {exit} |", exit.code())), "exit {exit}");
    }
    for word in [".config/farm.toml", "[job.", "cargo install --path tools/farm --locked", "--version"] {
        assert!(doc.contains(word), "{word}");
    }
    for word in [
        "--detach",
        "--timeout",
        "--limit",
        "--host",
        "--json",
        "--verbose",
        "--tail",
        "--at",
        "--forget",
        "INK_FARM_CONFIG",
        "uses.farm/v1",
    ] {
        assert!(doc.contains(word), "{word}");
    }
}

#[test]
fn the_workflow_doc_names_the_cache_flags_keys_and_the_agent_budget() {
    let doc = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/workflow.md"))
        .unwrap();
    for word in [
        "--fresh",
        "--budget",
        "inputs = [",
        "cache = false",
        "[sync]",
        "[cache]",
        "[budget]",
        "full_checks_per_day",
        "max_age_hours",
        "max_entries",
        "passed earlier on this tree",
        "wedged?",
        "Heavy-command budget",
    ] {
        assert!(doc.contains(word), "{word}");
    }
    for key in ["name", "ssh", "slots", "arch", "golden", "mold", "dir"] {
        assert!(doc.contains(&format!("\n{key} = ")), "config key {key}");
    }
}
