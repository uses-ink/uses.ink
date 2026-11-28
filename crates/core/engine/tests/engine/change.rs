//! A build against its baseline: what became public, what moved, and each changed decision's
//! cause.

use engine::Compared;
use model::code::{self, AnyCode};
use model::decision::{Decision, File};
use model::record::{Against, Cause, Touched};
use model::{AnyDecision, Evidence, Problem, Subject};

use crate::support::{Pinned, Tree, path};

fn against() -> Against {
    Against::Nothing
}

fn causes(compared: &Compared) -> Vec<(AnyCode, Subject, Cause)> {
    compared
        .changes
        .decisions
        .iter()
        .map(|change| (change.code, change.subject.clone(), change.cause.clone()))
        .collect()
}

#[test]
fn the_same_tree_twice_changes_nothing() {
    let tree = Tree::new().file("index.md", "# Home\n").file("a.md", "# A\n");
    let record = tree.record(&Pinned::none());
    let compared = Compared::of(against(), Some(&record), &tree.record(&Pinned::none()));
    assert!(compared.changes.is_empty(), "{:?}", compared.changes);
    assert_eq!(compared.diagnostics, []);
}

#[test]
fn a_first_build_makes_every_published_file_newly_public() {
    let after = Tree::new().file("index.md", "# Home\n").file("a.md", "# A\n").record(&Pinned::none());
    let compared = Compared::of(against(), None, &after);
    let public: Vec<_> = compared.changes.public.iter().map(|routed| routed.path.as_str()).collect();
    assert_eq!(public, ["a.md", "index.md"]);
    assert!(compared.changes.decisions.is_empty(), "no decision is listed when all are new");
}

#[test]
fn an_edit_is_content_and_a_new_file_is_newly_public() {
    let before = Tree::new().file("index.md", "# Home\n").file("a.md", "# A\n").record(&Pinned::none());
    let after = Tree::new().file("index.md", "# Home\n").file("a.md", "# Ay\n").file("b.md", "# B\n");
    let compared = Compared::of(against(), Some(&before), &after.record(&Pinned::none()));
    let public: Vec<_> = compared.changes.public.iter().map(|routed| routed.path.as_str()).collect();
    assert_eq!(public, ["b.md"]);
    let found = causes(&compared);
    let a = Subject::File(path("a.md"));
    assert!(found.contains(&(AnyCode::Title, a, Cause::Content(Touched::Edited))), "{found:?}");
    assert!(found.contains(&(AnyCode::Title, Subject::File(path("b.md")), Cause::Content(Touched::Added))));
    assert!(found.contains(&(AnyCode::Nav, Subject::Site, Cause::Content(Touched::Elsewhere))), "{found:?}");
    assert_eq!(compared.diagnostics, []);
}

#[test]
fn a_removed_file_is_withdrawn_and_a_new_slug_moves_it() {
    let before = Tree::new().file("a.md", "# A\n").file("b.md", "# B\n").record(&Pinned::none());
    let after = Tree::new().file("a.md", "---\nslug: ay\n---\n# A\n");
    let compared = Compared::of(against(), Some(&before), &after.record(&Pinned::none()));
    let withdrawn: Vec<_> = compared.changes.withdrawn.iter().map(|routed| routed.path.as_str()).collect();
    assert_eq!(withdrawn, ["b.md"]);
    let moved = &compared.changes.moved;
    assert_eq!((moved.len(), moved[0].before.as_str(), moved[0].after.as_str()), (1, "/a", "/ay"));
    let gone = compared.changes.decisions.iter().find(|change| change.subject == Subject::File(path("b.md"))).unwrap();
    assert_eq!((gone.after.as_ref(), &gone.cause), (None, &Cause::Content(Touched::Removed)));
}

#[test]
fn a_pin_is_config() {
    let tree = Tree::new().file("a.md", "# A\n");
    let before = tree.record(&Pinned::none());
    let after = tree.record(&Pinned::none().file::<code::Title>("a.md", "Pinned".into()));
    let found = causes(&Compared::of(against(), Some(&before), &after));
    assert_eq!(found, [(AnyCode::Title, Subject::File(path("a.md")), Cause::Config)]);
}

#[test]
fn a_change_with_nothing_else_changed_is_inks_and_a_warning() {
    let tree = Tree::new().file("a.md", "# A\n");
    let mut before = tree.record(&Pinned::none());
    let title = before.decisions.iter_mut().find(|decision| decision.code() == AnyCode::Title).unwrap();
    let heading = title.evidence().clone();
    *title = AnyDecision::from(Decision::<code::Title>::new(File(path("a.md")), "Older".to_owned(), heading));
    let compared = Compared::of(against(), Some(&before), &tree.record(&Pinned::none()));
    assert_eq!(causes(&compared), [(AnyCode::Title, Subject::File(path("a.md")), Cause::Engine)]);
    let problem = &compared.diagnostics[0].problem;
    assert_eq!(problem, &Problem::EngineDecisionUnstable { decision: AnyCode::Title });
    assert!(matches!(compared.changes.decisions[0].after.as_ref().unwrap().evidence(), Evidence::Heading { .. }));
}

#[test]
fn a_deepened_clone_is_the_sources_and_no_warning() {
    use model::{Known, Provenance, When};
    let at: model::Timestamp = "2022-05-06T07:08:09Z".parse().unwrap();
    let from = Provenance::Filesystem;
    let shallow =
        Tree::new().file("posts/a.md", "x\n").created("posts/a.md", Known::NotAfter { value: at, from: from.clone() });
    let deep = Tree::new().file("posts/a.md", "x\n").created("posts/a.md", Known::Exact { value: at, from });
    let before = shallow.shallow().record(&Pinned::none());
    let after = deep.deep().record(&Pinned::none());
    let compared = Compared::of(against(), Some(&before), &after);
    let date = compared.changes.decisions.iter().find(|change| change.code == AnyCode::Date).unwrap();
    assert_eq!(date.cause, Cause::Ability { ability: model::Ability::History });
    assert_eq!(date.after.as_ref().unwrap().get::<code::Date>().unwrap().value, Some(When::Instant(at)));
    assert_eq!(compared.diagnostics, []);
}

#[test]
fn a_changed_format_is_blamed_only_for_what_it_read() {
    use model::record::{Plugin, PluginId};
    let tree = Tree::new().file("a.md", "# A\n").file("b.md", "# B\n");
    let before = tree.record(&Pinned::none());
    let mut after = Tree::new().file("a.md", "# A\n").record(&Pinned::none());
    after.inputs.plugins.push(Plugin::new(PluginId::obsidian(), model::Hash::of(b"another build")));
    let compared = Compared::of(against(), Some(&before), &after);
    let found = causes(&compared);
    let b = Subject::File(path("b.md"));
    let plugin = Cause::Plugin { plugin: PluginId::obsidian() };
    let title = found.iter().find(|(code, subject, _)| *code == AnyCode::Title && *subject == b).unwrap();
    assert_eq!(title.2, plugin, "the heading was read by the format: {found:?}");
    let publish = found.iter().find(|(code, subject, _)| *code == AnyCode::Publish && *subject == b).unwrap();
    assert_eq!(publish.2, Cause::Content(Touched::Removed), "a default is no reading: {found:?}");
}

#[test]
fn two_decisions_of_one_code_and_subject_in_a_record_are_each_compared() {
    let build = |text: &str| Tree::new().file("a.md", text).record(&Pinned::none());
    let title = |build: &model::record::Build| {
        build.decisions.iter().find(|decision| decision.code() == AnyCode::Title).unwrap().clone()
    };
    let (a, b) = (build("# A\n"), build("# B\n"));
    let mut earlier = a.clone();
    earlier.decisions.push(title(&b));
    let mut later = b.clone();
    later.decisions.push(title(&b));
    let compared = Compared::of(against(), Some(&earlier), &later);
    let titles = causes(&compared).into_iter().filter(|(code, ..)| *code == AnyCode::Title).count();
    assert_eq!(titles, 1, "the first title changed and the second did not: {:?}", compared.changes);
}
