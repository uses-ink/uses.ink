//! Workspaces made and forgotten.

use super::*;

fn tree(base: &Path) -> Tree {
    let root = base.join("r-ws").join("here");
    std::fs::create_dir_all(&root).unwrap();
    Tree::locate(&root).unwrap()
}

fn base(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("farm-ws-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).unwrap();
    base
}

#[test]
fn the_warmest_sibling_is_the_one_with_the_newest_target() {
    let base = base("warm");
    let tree = tree(&base);
    let old = base.join("r-ws").join("old");
    let new = base.join("r-ws").join("new");
    for dir in [&old, &new] {
        std::fs::create_dir_all(dir.join("target")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let made = base.join("r-ws").join("made");
    std::fs::create_dir_all(&made).unwrap();
    assert_eq!(Workspaces::new(&tree).warmest(&made).unwrap().file_name().unwrap(), "new");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn seeding_copies_the_target_and_says_how() {
    let base = base("seed");
    let from = base.join("from");
    let to = base.join("to");
    std::fs::create_dir_all(from.join("target").join("debug")).unwrap();
    std::fs::write(from.join("target").join("debug").join("x"), "x").unwrap();
    std::fs::create_dir_all(&to).unwrap();
    let seeded = Workspaces::seed(&from, &to).unwrap();
    assert!(matches!(seeded, Seeded::Reflink(_) | Seeded::Copy(_)));
    assert_eq!(std::fs::read_to_string(to.join("target").join("debug").join("x")).unwrap(), "x");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn forgetting_refuses_what_is_not_a_workspace_of_this_repo() {
    let base = base("forget");
    let tree = tree(&base);
    let plain = base.join("r-ws").join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let spaces = Workspaces::new(&tree);
    assert!(matches!(spaces.forget(&"plain".parse().unwrap(), &[]), Err(Fault::Foreign { .. })));
    assert!(matches!(spaces.forget(&"nothing".parse().unwrap(), &[]), Err(Fault::Missing { .. })));
    assert!(matches!(spaces.forget(&"here".parse().unwrap(), &[]), Err(Fault::Current { .. })));
    assert!(plain.exists(), "a refusal removes nothing");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn the_strays_are_the_files_jj_does_not_list_outside_target() {
    let base = base("strays");
    for dir in [".jj", ".git", "target/debug", "src"] {
        std::fs::create_dir_all(base.join(dir)).unwrap();
    }
    for file in ["Cargo.toml", "src/lib.rs", "src/notes.txt", ".env", ".jj/store", ".git/HEAD", "target/debug/x"] {
        std::fs::write(base.join(file), "x").unwrap();
    }
    let strays = Strays::find(&base, "Cargo.toml\nsrc/lib.rs\n", &[]).unwrap();
    assert_eq!(strays, Strays(vec![".env".to_owned(), "src/notes.txt".to_owned()]));
    assert!(Strays::find(&base, ".env\nCargo.toml\nsrc/lib.rs\nsrc/notes.txt\n", &[]).unwrap().is_empty());
    std::fs::create_dir_all(base.join("site/.ink/cache")).unwrap();
    std::fs::write(base.join("site/.ink/cache/blob"), "x").unwrap();
    std::fs::write(base.join(".ink-note"), "x").unwrap();
    let ink: Vec<Disposable> = vec![".ink".parse().unwrap(), "notes.txt".parse().unwrap()];
    let kept = Strays::find(&base, "Cargo.toml\nsrc/lib.rs\n", &ink).unwrap();
    assert_eq!(
        kept,
        Strays(vec![".env".to_owned(), ".ink-note".to_owned()]),
        "`.ink` at any depth and a file name are skipped, `.env` is not"
    );
    let text = Fault::Untracked { name: "w".parse().unwrap(), paths: kept.into_paths() }.to_string();
    assert!(text.contains("`.env`") && text.contains("disposable"), "{text}");
    assert!(Fault::Untracked { name: "w".parse().unwrap(), paths: vec![] }.is_refusal());
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn a_symlink_out_of_the_container_is_not_removed() {
    let base = base("link");
    let tree = tree(&base);
    let elsewhere = base.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, base.join("r-ws").join("sneaky")).unwrap();
    let err = Workspaces::new(&tree).forget(&"sneaky".parse().unwrap(), &[]).unwrap_err();
    assert!(matches!(err, Fault::Outside { .. }), "{err}");
    assert!(elsewhere.exists());
    std::fs::remove_dir_all(&base).unwrap();
}
