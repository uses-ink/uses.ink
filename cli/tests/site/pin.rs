//! `ink pin`.

use super::*;

/// `ink pin` writes the decision's current value into `ink.toml`, keeping what was there, and
/// the site builds the same deploy again: a pin of what inference already says changes nothing.
#[test]
fn a_pin_of_the_current_value_changes_no_deploy() {
    let dir = scratch("pin/site");
    copy(&fixtures().join("readme"), &dir);
    let mine = "# mine\nexclude = [\"drafts/**\"]\n";
    std::fs::write(dir.join("ink.toml"), mine).unwrap();
    let before = build(&dir, &scratch("pin/before"));
    let dir_text = dir.to_str().unwrap();
    let (exit, dry, stderr) = ink(&["pin", "usage.md", "page.title", "--dry-run", "-C", dir_text]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(dry.contains("Would pin  page.title of usage.md; ink.toml is unchanged\n"), "{dry}");
    assert!(dry.contains("  @@ ink.toml\n   [page.\"usage.md\"]\n  +title = \"Usage\"\n"), "{dry}");
    assert_eq!(std::fs::read_to_string(dir.join("ink.toml")).unwrap(), mine);
    let (exit, _, stderr) = ink(&["pin", "usage.md", "page.title", "-C", dir_text]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    let (exit, _, stderr) = ink(&["pin", "site.title", "-C", dir_text]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    let toml = std::fs::read_to_string(dir.join("ink.toml")).unwrap();
    assert!(toml.starts_with(mine), "{toml}");
    assert!(toml.contains("[page.\"usage.md\"]\ntitle = \"Usage\""), "{toml}");
    assert!(toml.contains("[site]\ntitle = \"My tool\""), "{toml}");
    let (_, why, _) = ink(&["explain", "usage.md", "--why", "page.title", "-C", dir_text]);
    assert!(why.starts_with("page.title on usage.md\n╭─ “Usage”, pinned in ink.toml"), "{why}");
    assert_eq!(build(&dir, &scratch("pin/after")), before);
}

#[test]
fn pin_refuses_a_code_of_the_other_scope_or_an_unknown_file() {
    let dir = fixtures().join("readme");
    let dir_text = dir.to_str().unwrap();
    for (args, said) in [
        (
            &["pin", "page.title"][..],
            "error[decision.code.scope]: `page.title` is decided per file: give the file's path before it\n  fix: give the file's path before the code\n  nothing was changed\n",
        ),
        (
            &["pin", "usage.md", "site.title"][..],
            "error[decision.code.scope]: `site.title` is decided once for the site, not on usage.md: give the code alone\n  fix: give the code alone, without a path\n  nothing was changed\n",
        ),
        (
            &["pin", "nosuch.md", "page.title"][..],
            "error[explain.path.unknown]: nosuch.md is not a file of the folder\n  fix: `ink explain` lists every file of the folder and what is decided on it\n  nothing was changed\n",
        ),
        (
            &["pin", "page.nope"][..],
            "error[decision.code.unknown]: no decision has that code\n  caused by\n  ╰─ `page.nope` is no decision code\n  fix: `ink explain` lists every decision of the site with its code\n  nothing was changed\n",
        ),
    ] {
        let args: Vec<&str> = args.iter().copied().chain(["--dry-run", "-C", dir_text]).collect();
        let (exit, stdout, stderr) = ink(&args);
        assert_eq!((exit, stdout.as_str(), stderr.as_str()), (Exit::Usage, "", said), "{args:?}");
    }
}
