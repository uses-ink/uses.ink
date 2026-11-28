//! `ink explain`.

use super::*;

#[test]
fn explain_why_replays_one_ladder_rung_by_rung() {
    let dir = fixtures().join("blog");
    let (exit, why, stderr) = ink(&["explain", "posts/later.md", "--why", "page.title", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(
        why.starts_with("page.title on posts/later.md\n╭─ “Later”, from the name of posts/later.md: rung 4 of 4\n│\n"),
        "{why}"
    );
    for rung in ["│ · 1  ink.toml", "│ · 2  front matter", "│ · 3  first heading", "│ ✓ 4  file name"] {
        assert!(why.contains(rung), "{rung} in {why}");
    }
    assert!(why.ends_with("╰─ to pin it: ink pin posts/later.md page.title\n"), "{why}");
    let (_, why, _) = ink(&["explain", "index.md", "--why", "page.summary", "-C", dir.to_str().unwrap()]);
    assert!(why.contains("– 3  first paragraph"), "the outranked rung is shown: {why}");
    assert!(why.lines().any(|line| line.starts_with("│ – 3") && line.contains("outranked")), "{why}");
    let (_, json, _) = ink(&["explain", "--why", "site.layout", "-C", dir.to_str().unwrap(), "--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema"], "uses.trace/v1");
    assert_eq!(json["trace"]["code"], "site.layout");
    assert_eq!(json["trace"]["rungs"][3]["witness"], serde_json::json!({ "folder": "posts" }));
    assert_eq!(json["trace"]["rungs"][3]["heard"][0]["chosen"], true);
}

#[test]
fn explain_shows_decisions_with_evidence() {
    let dir = fixtures().join("readme");
    let (exit, stdout, _) = ink(&["explain", "-C", dir.to_str().unwrap(), "usage.md"]);
    assert_eq!(exit, Exit::Done);
    assert!(stdout.starts_with("usage.md\n"), "{stdout}");
    let title = stdout.lines().find(|line| line.contains("page.title")).unwrap();
    assert!(title.contains("“Usage”") && title.contains("from the heading of usage.md, bytes 0..7"), "{stdout}");
    let (_, json, _) = ink(&["explain", "-C", dir.to_str().unwrap(), "--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema"], "uses.explain/v2");
    assert!(json["decisions"].as_array().unwrap().len() > 10);
    let format = &json["decisions"][0];
    assert_eq!(format["code"], "site.format", "decisions are tagged by their code: {format}");
    assert_eq!((format["subject"].as_str(), format["value"].as_str()), (Some("site"), Some("ink:format-markdown")));
    let root = &json["decisions"][1];
    assert_eq!(root["code"], "site.root", "decisions are tagged by their code: {root}");
    assert_eq!(root["subject"], "site");
    assert_eq!(root["value"], "top");
}

/// `ink explain` on every site, compared with `fixture/explain/<name>.snap`. Not its JSON: that
/// holds the file times the folder reports.
#[test]
fn explain_shows_every_site_as_expected() {
    let cases = sites().into_iter().map(|name| {
        let (_, lines, _) = ink(&["explain", "-C", fixtures().join(&name).to_str().unwrap()]);
        (name, lines)
    });
    fixture::cases("fixture/explain", cases);
}

#[test]
fn explain_refuses_a_path_that_is_no_file() {
    let dir = fixtures().join("readme");
    let (exit, stdout, stderr) = ink(&["explain", "-C", dir.to_str().unwrap(), "nosuch.md"]);
    assert_eq!(exit, Exit::Usage);
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "error[explain.path.unknown]: nosuch.md is not a file of the folder\n  fix: `ink explain` lists every file of the folder and what is decided on it\n  nothing was changed\n"
    );
    let (exit, _, _) = ink(&["explain", "nosuch.md", "--why", "page.title", "-C", dir.to_str().unwrap()]);
    assert_eq!(exit, Exit::Usage);
}
