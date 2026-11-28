//! `ink check`.

use super::*;

#[test]
fn check_exits_by_what_it_found() {
    let check = |name: &str, strict: &[&str]| {
        let dir = fixtures().join(name);
        let args: Vec<&str> =
            ["check", "-C", dir.to_str().unwrap()].into_iter().chain(strict.iter().copied()).collect();
        ink(&args)
    };
    let (exit, stdout, stderr) = check("readme", &[]);
    assert_eq!(exit, Exit::Done);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "     Checked  2 pages, 30 decisions: no errors, no warnings\n");
    let (exit, _, stderr) = check("ambiguous", &[]);
    assert_eq!(exit, Exit::Content);
    assert!(stderr.starts_with("error[link.target.ambiguous]:"), "{stderr}");
    let (exit, _, stderr) = check("broken-frontmatter", &["--strict"]);
    assert_eq!(exit, Exit::Strict);
    assert!(stderr.contains("warning[frontmatter.syntax.invalid]"), "{stderr}");
    assert!(stderr.contains("2 | / title: [unclosed"), "the source line is under the diagnostic: {stderr}");
}
