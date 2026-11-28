//! Removing and pruning copies, over the fake transport.

use super::*;
use crate::cli::exit::Exit;
use crate::host::fake::{Fake, Reply};
use crate::host::transport::program::Program;

fn tree() -> Tree {
    let base = std::env::temp_dir().join(format!("farm-copies-{}", std::process::id()));
    std::fs::create_dir_all(base.join("r-ws").join("alive")).unwrap();
    Tree { root: base.join("r-ws").join("alive"), repo: "r".parse().unwrap(), workspace: "alive".parse().unwrap() }
}

fn member(name: &str, replies: Vec<Reply>) -> Member<Fake> {
    Member::new(Fake::remote(name), Fake::saying(replies))
}

fn name(text: &str) -> Segment {
    text.parse().unwrap()
}

#[test]
fn forgetting_removes_the_copy_on_every_reachable_host_and_says_which_were_not() {
    let up = member("petal", vec![Reply::ok(&["@@farm:removed 3145728"])]);
    let none = member("farm-a", vec![Reply::ok(&["@@farm:absent"])]);
    let down = member("farm-b", vec![Reply { lines: vec![], code: 255 }]);
    let local = Member::new(crate::host::Host::local(), Fake { local: true, ..Fake::default() });
    let fleet = Fleet::new(vec![up, none, down, local]);
    let tree = tree();
    let removal = Copies::new(&fleet, &tree).forget(&name("old"));
    let text = removal.to_string();
    assert!(text.contains("petal: removed ~/farm/r/old (3.0 GiB)"), "{text}");
    assert!(text.contains("farm-a: no copy at ~/farm/r/old"), "{text}");
    assert!(text.contains("farm-b: not reached, so ~/farm/r/old may remain"), "{text}");
    assert!(text.contains("freed 3.0 GiB"), "{text}");
    assert_eq!(removal.exit(), Exit::Passed);
    let asked = fleet.members().first().unwrap().transport().calls.lock().unwrap().clone();
    assert_eq!(asked[0].program, Program::Remove);
    assert_eq!(asked[0].args, ["farm", "r", "old"]);
    assert!(fleet.members().last().unwrap().transport().calls.lock().unwrap().is_empty(), "this machine is not asked");
}

#[test]
fn a_path_the_host_resolves_outside_its_directory_is_refused_loudly_and_the_exit_is_refused() {
    let odd = member("petal", vec![Reply::ok(&["@@farm:refused /srv/elsewhere"])]);
    let fleet = Fleet::new(vec![odd]);
    let tree = tree();
    let removal = Copies::new(&fleet, &tree).forget(&name("old"));
    let text = removal.to_string();
    assert!(text.contains("refused: ~/farm/r/old resolves to /srv/elsewhere"), "{text}");
    assert!(text.contains("nothing was removed"), "{text}");
    assert_eq!(removal.exit(), Exit::Refused);
}

#[test]
fn a_survey_lists_only_the_copies_no_workspace_here_owns() {
    let listing =
        Reply::ok(&["@@farm:copy alive 10", "@@farm:copy main 20", "@@farm:copy gone 2097152", "@@farm:copy lost 5"]);
    let fleet =
        Fleet::new(vec![member("petal", vec![listing]), member("farm-a", vec![Reply { lines: vec![], code: 255 }])]);
    let tree = tree();
    let owners = Owners::of(&tree).unwrap();
    let survey = Copies::new(&fleet, &tree).survey(&owners);
    let found: Vec<&str> = survey.strays.iter().map(|stray| stray.name.as_str()).collect();
    assert_eq!(found, ["gone", "lost"]);
    let text = survey.to_string();
    assert!(text.contains("petal: ~/farm/r/gone (2.0 GiB)"), "{text}");
    assert!(text.contains("2 copies, 2.0 GiB, that no workspace here owns"), "{text}");
    assert!(text.contains("farm-a: not reached, its copies were not looked at"), "{text}");
    for member in fleet.members() {
        let calls = member.transport().calls.lock().unwrap();
        assert!(calls.iter().all(|call| call.program == Program::List), "a survey removes nothing");
    }
}

#[test]
fn pruning_removes_exactly_the_surveyed_copies_on_their_hosts() {
    let listing = Reply::ok(&["@@farm:copy gone 1", "@@farm:copy alive 1"]);
    let removed = Reply::ok(&["@@farm:removed 1"]);
    let fleet = Fleet::new(vec![member("petal", vec![listing, removed])]);
    let tree = tree();
    let copies = Copies::new(&fleet, &tree);
    let survey = copies.survey(&Owners::of(&tree).unwrap());
    let removal = copies.prune(&survey);
    assert!(removal.to_string().contains("petal: removed ~/farm/r/gone"), "{removal}");
    let calls = fleet.members().first().unwrap().transport().calls.lock().unwrap().clone();
    let asked: Vec<&Vec<String>> =
        calls.iter().filter(|call| call.program == Program::Remove).map(|call| &call.args).collect();
    assert_eq!(asked, [&vec!["farm".to_owned(), "r".to_owned(), "gone".to_owned()]]);
}

#[test]
fn nothing_to_prune_says_so() {
    assert_eq!(Survey::default().to_string(), "no host holds a copy that no workspace here owns");
    assert!(Survey::default().is_empty());
}

/// The real `Remove` program on a real directory: it removes a workspace, refuses a symlink and
/// a name that is not one segment, and never touches what a symlink points at.
mod program {
    use std::process::Command;

    use crate::host::transport::program::Program;

    fn run(farm: &std::path::Path, repo: &str, ws: &str) -> String {
        let out =
            Command::new("sh").args(["-c", Program::Remove.text(), "sh"]).arg(farm).args([repo, ws]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }

    #[test]
    fn it_removes_a_workspace_directory_and_nothing_outside_the_farm() {
        let base = std::env::temp_dir().join(format!("farm-remove-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let (farm, elsewhere) = (base.join("farm"), base.join("elsewhere"));
        std::fs::create_dir_all(farm.join("r").join("old").join("target")).unwrap();
        std::fs::write(farm.join("r").join("old").join("target").join("x"), "x".repeat(4096)).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("precious"), "keep").unwrap();
        std::os::unix::fs::symlink(&elsewhere, farm.join("r").join("link")).unwrap();
        assert!(run(&farm, "r", "old").starts_with("@@farm:removed "), "a real directory goes");
        assert!(!farm.join("r").join("old").exists());
        assert!(run(&farm, "r", "old").starts_with("@@farm:absent"), "a second time there is none");
        assert!(run(&farm, "r", "link").starts_with("@@farm:refused "), "a symlink is never followed");
        for bad in ["..", "a/b", ".", "-rf"] {
            assert!(run(&farm, "r", bad).starts_with("@@farm:refused "), "{bad}");
        }
        assert_eq!(std::fs::read_to_string(elsewhere.join("precious")).unwrap(), "keep");
        assert!(farm.join("r").join("link").is_symlink());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_repo_directory_that_is_a_symlink_out_of_the_farm_is_refused() {
        let base = std::env::temp_dir().join(format!("farm-remove-repo-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let (farm, elsewhere) = (base.join("farm"), base.join("elsewhere"));
        std::fs::create_dir_all(&farm).unwrap();
        std::fs::create_dir_all(elsewhere.join("old")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, farm.join("r")).unwrap();
        assert!(run(&farm, "r", "old").starts_with("@@farm:refused "));
        assert!(elsewhere.join("old").is_dir());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn the_list_names_directories_the_farm_would_make_and_skips_half_made_seeds() {
        let base = std::env::temp_dir().join(format!("farm-list-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let farm = base.join("farm");
        for dir in ["a", "b.seed", ".hidden", "with space"] {
            std::fs::create_dir_all(farm.join("r").join(dir)).unwrap();
        }
        let out = Command::new("sh").args(["-c", Program::List.text(), "sh"]).arg(&farm).arg("r").output().unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.starts_with("@@farm:copy a "), "{text}");
        std::fs::remove_dir_all(&base).unwrap();
    }
}
