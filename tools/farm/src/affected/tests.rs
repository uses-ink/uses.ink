//! Which crates a change affects, over a fake `cargo metadata` and a fake list of changed files.

use serde_json::json;

use super::choice::Reason;
use super::touched::{Listing, Touched};
use super::*;

/// A workspace at `/w`: `(name, dir, dependencies)`; a dependency `dev:x` is a dev edge.
struct Fixture(Vec<(&'static str, &'static str, Vec<&'static str>)>);

impl Fixture {
    fn json(&self) -> String {
        let packages: Vec<_> = self
            .0
            .iter()
            .map(|(name, dir, deps)| {
                let deps: Vec<_> = deps
                    .iter()
                    .map(|dep| match dep.strip_prefix("dev:") {
                        Some(name) => json!({"name": name, "kind": "dev", "rename": null, "path": "/x"}),
                        None => json!({"name": dep, "kind": null, "req": "*"}),
                    })
                    .collect();
                let manifest = if dir.is_empty() { "/w/Cargo.toml".to_owned() } else { format!("/w/{dir}/Cargo.toml") };
                json!({"name": name, "version": "0.1.0", "id": "x", "manifest_path": manifest, "dependencies": deps})
            })
            .collect();
        json!({"packages": packages, "workspace_root": "/w", "version": 1}).to_string()
    }
}

impl Layout for Fixture {
    fn json(&self) -> Result<String, Fault> {
        Ok(Self::json(self))
    }
}

/// A fixed list of changed paths.
struct Fixed(Vec<&'static str>);

impl Touch for Fixed {
    fn paths(&self, _: &Base) -> Result<Vec<Touched>, Fault> {
        self.0.iter().map(|path| Touched::read(path)).collect()
    }
}

fn world() -> Fixture {
    Fixture(vec![
        ("model", "model", vec![]),
        ("engine", "engine", vec!["model"]),
        ("cli", "cli", vec!["engine", "model"]),
        ("web", "web", vec!["model"]),
        ("fixture", "fixture", vec![]),
        ("tests", "tests", vec!["dev:fixture"]),
        ("edge", "edge", vec![]),
        ("api", "edge/api", vec!["model"]),
        ("farm", "tools/farm", vec![]),
    ])
}

fn scope() -> Scope {
    let ignore = Selection::parse(&["docs/**".to_owned(), "*.md".to_owned()]).unwrap();
    Scope { base: None, ignore }
}

fn choose(changed: Vec<&'static str>) -> Choice {
    let (layout, touch, scope) = (world(), Fixed(changed), scope());
    Affected { layout: &layout, touch: &touch, scope: &scope }.choose(&Base::default()).unwrap()
}

fn names(list: &[Package]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

#[test]
fn a_leaf_crate_affects_only_itself() {
    let Choice::Some { changed, dependents, total } = choose(vec!["tools/farm/src/main.rs"]) else { panic!() };
    assert_eq!((names(&changed), dependents.len(), total), (["farm"].map(String::from).to_vec(), 0, 9));
}

#[test]
fn a_core_crate_affects_every_dependent_directly_or_not() {
    let choice = choose(vec!["model/src/lib.rs"]);
    let Choice::Some { changed, dependents, .. } = &choice else { panic!() };
    assert_eq!(names(changed), ["model"]);
    assert_eq!(names(dependents), ["api", "cli", "engine", "web"]);
    assert_eq!(choice.to_string(), "affected: 5 of 9 crates (changed: model; dependents: 4)");
    assert_eq!(
        choice.flags().unwrap(),
        ["-p", "model", "-p", "api", "-p", "cli", "-p", "engine", "-p", "web"].map(String::from)
    );
}

#[test]
fn a_transitive_dependent_is_reached_through_a_middle_crate() {
    let Choice::Some { dependents, .. } = choose(vec!["engine/src/a.rs"]) else { panic!() };
    assert_eq!(names(&dependents), ["cli"]);
}

#[test]
fn a_dev_dependency_edge_counts() {
    let Choice::Some { dependents, .. } = choose(vec!["fixture/data.json"]) else { panic!() };
    assert_eq!(names(&dependents), ["tests"]);
}

#[test]
fn a_nested_crate_owns_its_files_before_the_crate_around_it() {
    let Choice::Some { changed, .. } = choose(vec!["edge/api/src/lib.rs"]) else { panic!() };
    assert_eq!(names(&changed), ["api"]);
    let Choice::Some { changed, .. } = choose(vec!["edge/src/lib.rs"]) else { panic!() };
    assert_eq!(names(&changed), ["edge"]);
}

#[test]
fn a_shared_file_means_all() {
    for path in
        ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".config/nextest.toml", ".config/farm.toml", "deny.toml"]
    {
        let choice = choose(vec!["model/src/lib.rs", path]);
        assert!(matches!(&choice, Choice::Everything { why: Reason::Shared(_), total: 9 }), "{path}: {choice:?}");
    }
    assert_eq!(choose(vec!["Cargo.lock"]).flags().unwrap(), ["--workspace"]);
    assert!(matches!(choose(vec!["model/Cargo.toml"]), Choice::Some { .. }), "a crate's own manifest is the crate's");
}

#[test]
fn docs_only_means_none() {
    let choice = choose(vec!["docs/workflow.md", "PRODUCT.md", "docs/img/a.png"]);
    assert_eq!(choice, Choice::Nothing);
    assert_eq!(choice.flags(), None);
    assert_eq!(choice.to_string(), "no crate affected");
    assert_eq!(choose(vec![]), Choice::Nothing);
}

#[test]
fn a_path_in_no_crate_that_is_not_ignored_means_all() {
    let choice = choose(vec!["docs/a.md", "newdir/new.rs"]);
    assert!(
        matches!(&choice, Choice::Everything { why: Reason::Unowned(path), .. } if path.as_str() == "newdir/new.rs")
    );
    assert_eq!(choice.to_string(), "affected: all 9 crates (`newdir/new.rs` is in no crate)");
}

#[test]
fn a_rename_counts_both_paths() {
    let touched =
        Listing::read("R model/src/{a.rs => b.rs}\nR {web => cli}/x.rs\nR old.rs => fixture/new.rs\nM docs/a.md\n")
            .unwrap();
    let all: Vec<&str> = touched.iter().map(Touched::as_str).collect();
    assert_eq!(
        all,
        ["model/src/a.rs", "model/src/b.rs", "web/x.rs", "cli/x.rs", "old.rs", "fixture/new.rs", "docs/a.md"]
    );
    let graph = Graph::read(&world().json()).unwrap();
    let Choice::Some { changed, .. } = Choice::of(&graph, &touched[2..4], &scope().ignore) else { panic!() };
    assert_eq!(names(&changed), ["cli", "web"], "the old crate and the new crate");
}

#[test]
fn a_move_into_a_new_directory_reads_both_sides() {
    let touched = Listing::read("R model/{ => sub}/a.rs\nA x/y.rs\nD gone.rs\n").unwrap();
    let all: Vec<&str> = touched.iter().map(Touched::as_str).collect();
    assert_eq!(all, ["model/a.rs", "model/sub/a.rs", "x/y.rs", "gone.rs"]);
}

#[test]
fn a_summary_line_or_a_path_that_is_not_safe_is_refused() {
    for bad in ["Z a.rs", "nospace", "M /etc/passwd", "M ../x", "R {a => b", "M a//b"] {
        assert!(Listing::read(bad).is_err(), "{bad}");
    }
}

#[test]
fn metadata_that_is_not_json_or_whose_manifest_is_elsewhere_is_a_typed_fault() {
    assert!(matches!(Graph::read("not json"), Err(Fault::Json(_))));
    let odd = json!({"packages": [{"name": "a", "manifest_path": "/elsewhere/a/Cargo.toml"}], "workspace_root": "/w"});
    assert!(matches!(Graph::read(&odd.to_string()), Err(Fault::Manifest { .. })));
}

#[test]
fn a_root_package_owns_what_no_other_crate_does() {
    let world = Fixture(vec![("top", "", vec![]), ("sub", "sub", vec![])]);
    let graph = Graph::read(&world.json()).unwrap();
    assert_eq!(graph.owner("sub/a.rs"), Some(1));
    assert_eq!(graph.owner("src/lib.rs"), Some(0));
}

#[test]
fn the_line_lists_six_names_and_counts_the_rest() {
    let names: Vec<Package> = (0..9).map(|n| Package::named(&format!("c{n}"))).collect();
    let choice = Choice::Some { changed: names, dependents: vec![], total: 20 };
    assert_eq!(
        choice.to_string(),
        "affected: 9 of 20 crates (changed: c0, c1, c2, c3, c4, c5, and 3 more; dependents: 0)"
    );
}

#[test]
fn a_base_is_a_revision_not_a_flag() {
    assert!("main".parse::<Base>().is_ok());
    assert!("-x".parse::<Base>().is_err() && "".parse::<Base>().is_err());
    assert_eq!(Base::default().as_str(), "heads(::main & ::@)");
}
