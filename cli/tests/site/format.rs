//! `site.format`: chosen, pinned and refused.

use super::*;

/// `site.format` is a pin, then the `.obsidian/` marker, then markdown, each replayed by `--why`,
/// and a pin is said.
#[test]
fn the_format_is_a_pin_then_the_vault_marker_then_markdown() {
    let why = |name: &str| {
        let dir = fixtures().join(name);
        let (exit, why, stderr) = ink(&["explain", "--why", "site.format", "-C", dir.to_str().unwrap(), "-v"]);
        assert_eq!(exit, Exit::Done, "{stderr}");
        (why, stderr)
    };
    let (why_vault, said) = why("vault");
    assert!(
        why_vault.starts_with(
            "site.format on the site\n╭─ `ink:format-obsidian`, because .obsidian is there: rung 2 of 3\n│\n"
        ),
        "{why_vault}"
    );
    for rung in ["│ · 1  ink.toml", "│ ✓ 2  .obsidian folder", "│ · 3  default"] {
        assert!(why_vault.contains(rung), "{rung} in {why_vault}");
    }
    assert!(why_vault.ends_with("╰─ to pin it: ink pin site.format\n"), "{why_vault}");
    assert!(said.contains("note[format.obsidian.chosen]"), "{said}");
    let (plain, said) = why("readme");
    assert!(plain.starts_with("site.format on the site\n╭─ `ink:format-markdown`, by default: rung 3 of 3"), "{plain}");
    assert!(!said.contains("format."), "markdown by default is not said: {said}");
    let (pinned, said) = why("vault-markdown");
    assert!(
        pinned.starts_with(
            "site.format on the site\n╭─ `ink:format-markdown`, pinned in ink.toml, bytes 7..37: rung 1 of 3\n│\n"
        ),
        "{pinned}"
    );
    assert!(pinned.contains("│ – 2  .obsidian folder"), "{pinned}");
    assert!(pinned.contains("outranked"), "{pinned}");
    assert!(said.contains("note[format.pin.chosen]: `ink.toml` pins `site.format` to `ink:format-markdown`"), "{said}");
    assert!(!said.contains("format.obsidian.chosen"), "{said}");
    let (pinned, said) = why("notes-obsidian");
    assert!(pinned.starts_with("site.format on the site\n╭─ `ink:format-obsidian`, pinned in ink.toml"), "{pinned}");
    assert!(said.contains("note[format.pin.chosen]: `ink.toml` pins `site.format` to `ink:format-obsidian`"), "{said}");
}

/// A pinned format reads the files: a vault pinned to markdown leaves `#todo` text and a folder
/// pinned to Obsidian reads it as a tag; `ink pin site.format` writes what is there.
#[test]
fn a_pinned_format_reads_the_markdown_and_ink_pin_writes_it() {
    let tags = |name: &str| {
        let dir = fixtures().join(name);
        let (exit, json, stderr) = ink(&["explain", "-C", dir.to_str().unwrap(), "--json"]);
        assert_eq!(exit, Exit::Done, "{stderr}");
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = json["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "page.tags" && d["subject"]["file"] == "index.md")
            .cloned();
        tags.unwrap()
    };
    assert_eq!(tags("vault-markdown")["value"], serde_json::json!([]));
    let obsidian = tags("notes-obsidian");
    assert_eq!(obsidian["value"][0]["written"], "todo");
    assert_eq!(obsidian["evidence"]["inline"]["path"], "index.md", "{obsidian}");
    let dir = scratch("pin-format/site");
    copy(&fixtures().join("vault"), &dir);
    let (exit, written, stderr) = ink(&["pin", "site.format", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(written.contains("  +format = \"ink:format-obsidian\"\n"), "{written}");
    assert_eq!(std::fs::read_to_string(dir.join("ink.toml")).unwrap(), "[site]\nformat = \"ink:format-obsidian\"\n");
    let (exit, _, stderr) = ink(&["explain", "--why", "site.format", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Done, "{stderr}");
}

#[test]
fn a_format_this_ink_does_not_hold_stops_the_command() {
    let dir = scratch("format-unknown/site");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ink.toml"), "[site]\nformat = \"acme:format-wiki\"\n").unwrap();
    std::fs::write(dir.join("index.md"), "# Home\n").unwrap();
    let (exit, _, stderr) = ink(&["check", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Usage, "{stderr}");
    assert!(
        stderr.starts_with(
            "error[format.plugin.unknown]: `site.format` names `acme:format-wiki`, which this ink does not hold"
        ),
        "{stderr}"
    );
    std::fs::write(dir.join("ink.toml"), "[site]\nformat = \"quiet\"\n").unwrap();
    let (exit, _, stderr) = ink(&["check", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Content, "{stderr}");
    assert!(stderr.contains("config.value.invalid"), "{stderr}");
}
