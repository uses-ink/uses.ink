//! Each inferrer's precedence: a pin beats frontmatter, frontmatter beats the file name, the file
//! name beats a default; and the defaults say so.

use jiff::civil::date;
use model::code::{self, AnyCode};
use model::{
    DefaultReason, Evidence, Known, Layout, PageKind, Provenance, Rejection, Root, Slug, Subject, Timestamp, When,
};

use crate::support::{Pinned, Tree, codes, on, path, site, subject};

fn instant(text: &str) -> Timestamp {
    text.parse().unwrap()
}

#[test]
fn title_pin_beats_frontmatter_beats_heading_beats_file_name() {
    let tree = Tree::new()
        .file("a.md", "---\ntitle: Front\n---\n# Heading\n")
        .file("b.md", "# Heading\n")
        .file("on-shelves.md", "text\n");
    let none = tree.understand(&Pinned::none());
    let title = |u, p| on::<code::Title>(u, p);
    let a = title(&none, "a.md");
    assert_eq!(a.value, "Front");
    assert!(matches!(a.evidence, Evidence::Frontmatter { .. }));
    assert!(a.rejected.iter().all(|alt| alt.because == Rejection::Outranked));
    assert!(matches!(a.rejected[0].evidence, Evidence::Heading { .. }));
    assert!(matches!(title(&none, "b.md").evidence, Evidence::Heading { .. }));
    let named = title(&none, "on-shelves.md");
    assert_eq!(
        (named.value, named.evidence),
        ("On shelves".to_owned(), Evidence::Filename { path: path("on-shelves.md") })
    );
    let pinned = tree.understand(&Pinned::none().file::<code::Title>("a.md", "Pinned".into()));
    let a = title(&pinned, "a.md");
    assert_eq!(a.value, "Pinned");
    assert!(matches!(a.evidence, Evidence::Config { .. }));
    assert_eq!(a.rejected.len(), 2);
}

#[test]
fn date_takes_frontmatter_then_file_name_then_exact_history() {
    let commit = Provenance::Filesystem;
    let tree = Tree::new()
        .file("posts/2024-01-02-a.md", "---\ndate: 2024-03-01\n---\n")
        .file("posts/2024-01-02-b.md", "x\n")
        .file("posts/c.md", "x\n")
        .created("posts/c.md", Known::Exact { value: instant("2023-05-06T07:08:09Z"), from: commit.clone() });
    let u = tree.understand(&Pinned::none());
    let date_of = |p| on::<code::Date>(&u, p).value;
    assert_eq!(date_of("posts/2024-01-02-a.md"), Some(When::Day(date(2024, 3, 1))));
    assert_eq!(date_of("posts/2024-01-02-b.md"), Some(When::Day(date(2024, 1, 2))));
    assert_eq!(date_of("posts/c.md"), Some(When::Instant(instant("2023-05-06T07:08:09Z"))));
    assert!(matches!(on::<code::Date>(&u, "posts/c.md").evidence, Evidence::History { .. }));
    let pinned = tree.understand(&Pinned::none().file::<code::Date>("posts/2024-01-02-a.md", None));
    let a = on::<code::Date>(&pinned, "posts/2024-01-02-a.md");
    assert_eq!(a.value, None);
    assert_eq!(a.rejected.len(), 2);
}

#[test]
fn a_bound_is_never_a_date() {
    let bound = Known::NotAfter { value: instant("2023-05-06T07:08:09Z"), from: Provenance::Filesystem };
    let u = Tree::new().file("posts/c.md", "x\n").created("posts/c.md", bound).understand(&Pinned::none());
    let c = on::<code::Date>(&u, "posts/c.md");
    assert_eq!(c.value, None);
    assert_eq!(c.evidence, Evidence::Default { reason: DefaultReason::OnlyBound });
    assert_eq!(c.rejected[0].because, Rejection::OnlyBound);
    assert!(u.notices.iter().any(|n| n.code() == "fact.date.bound"));
}

#[test]
fn kind_takes_type_then_folder_then_date() {
    let u = Tree::new()
        .file("posts/a.md", "---\ntype: page\n---\n")
        .file("posts/b.md", "x\n")
        .file("c.md", "---\ndate: 2024-03-01\n---\n")
        .file("d.md", "x\n")
        .understand(&Pinned::none());
    let kind = |p| on::<code::Kind>(&u, p).value;
    assert_eq!(kind("posts/a.md"), PageKind::Page);
    assert_eq!(kind("posts/b.md"), PageKind::Post);
    assert_eq!(kind("c.md"), PageKind::Post);
    assert_eq!(kind("d.md"), PageKind::Page);
    assert!(u.notices.iter().any(|n| n.code() == "notice.default.chosen" && n.subject == Some(subject("d.md"))));
}

#[test]
fn publish_takes_frontmatter_then_drafts_then_the_default() {
    let tree = Tree::new()
        .file("a.md", "---\ndraft: true\n---\n")
        .file("drafts/b.md", "x\n")
        .file("drafts/c.md", "---\npublish: true\n---\n")
        .file("_d.md", "x\n")
        .file("e.md", "x\n");
    let u = tree.understand(&Pinned::none().file::<code::Publish>("e.md", false));
    let publish = |p| on::<code::Publish>(&u, p).value;
    assert!(!publish("a.md"));
    assert!(!publish("drafts/b.md"));
    assert!(publish("drafts/c.md"));
    assert!(!publish("_d.md"));
    assert!(!publish("e.md"));
    let site = u.site.unwrap();
    assert_eq!(site.pages().iter().map(|p| p.file.as_str()).collect::<Vec<_>>(), ["drafts/c.md"]);
}

#[test]
fn a_vault_publishes_only_what_is_selected() {
    let tree = Tree::new()
        .file(".obsidian/app.json", "{}")
        .file("Public/a.md", "x\n")
        .file("Private/b.md", "x\n")
        .file("c.md", "---\npublish: true\n---\n");
    let u = tree.understand(&Pinned::none().include("Public/"));
    assert_eq!(site::<code::Layout>(&u).value, Layout::Vault);
    let b = on::<code::Publish>(&u, "Private/b.md");
    assert_eq!(b.evidence, Evidence::Default { reason: DefaultReason::NotSelected });
    let files: Vec<&str> = u.site.as_ref().unwrap().pages().iter().map(|p| p.file.as_str()).collect();
    assert_eq!(files, ["c.md", "Public/a.md"]);
    let private = Tree::new().private().file("a.md", "x\n").understand(&Pinned::none());
    assert_eq!(private.site.unwrap().pages(), []);
}

#[test]
fn ignore_takes_exclude_then_include_then_rules() {
    let tree = Tree::new()
        .file(".github/x.md", "x\n")
        .file("LICENSE.md", "x\n")
        .file("node_modules/y.md", "x\n")
        .file("notes/private.md", "x\n")
        .file(".well/kept.md", "x\n")
        .file("a.md", "x\n");
    let u = tree.understand(&Pinned::none().exclude("notes/").include(".well/"));
    let ignored = |p| on::<code::Ignored>(&u, p).value;
    for p in [".github/x.md", "LICENSE.md", "node_modules/y.md", "notes/private.md"] {
        assert!(ignored(p), "{p}");
    }
    assert!(!ignored(".well/kept.md"));
    assert!(!ignored("a.md"));
    let because = AnyCode::Ignored;
    let excluded = u.site.unwrap().excluded().iter().filter(|e| e.because == because).count();
    assert_eq!(excluded, 4);
}

#[test]
fn root_moves_into_the_only_content_folder() {
    let u = Tree::new().file("README.md", "x\n").file("content/a.md", "x\n").understand(&Pinned::none());
    assert_eq!(site::<code::Root>(&u).value, Root::Folder(path("content")));
    let built = u.site.unwrap();
    assert_eq!(built.pages()[0].route.as_str(), "/a");
    assert_eq!(built.excluded()[0].because, AnyCode::Root);
    let top = Tree::new().file("about.md", "x\n").file("content/a.md", "x\n").understand(&Pinned::none());
    assert_eq!(site::<code::Root>(&top).value, Root::Top);
    assert!(top.notices.iter().any(|n| n.code() == "notice.default.chosen" && n.subject == Some(Subject::Site)));
}

#[test]
fn slug_and_route_follow_the_name() {
    let u = Tree::new()
        .file("posts/2024-03-01-On Shelves.md", "x\n")
        .file("posts/b.md", "---\nslug: custom\n---\n")
        .file("guide/README.md", "x\n")
        .file("index.md", "x\n")
        .understand(&Pinned::none());
    let site = u.site.unwrap();
    let routes: Vec<(&str, &str)> = site.pages().iter().map(|p| (p.file.as_str(), p.route.as_str())).collect();
    assert_eq!(
        routes,
        [
            ("index.md", "/"),
            ("guide/README.md", "/guide"),
            ("posts/b.md", "/posts/custom"),
            ("posts/2024-03-01-On Shelves.md", "/posts/on-shelves")
        ]
    );
    assert_eq!(site.home().unwrap().kind, PageKind::Home);
    let pinned = Tree::new()
        .file("a.md", "x\n")
        .understand(&Pinned::none().file::<code::Slug>("a.md", Slug::parse("z").unwrap()));
    assert_eq!(pinned.site.unwrap().pages()[0].route.as_str(), "/z");
}

#[test]
fn home_takes_the_first_named_page_or_generates_a_listing() {
    let u = Tree::new().file("README.md", "x\n").file("index.md", "x\n").understand(&Pinned::none());
    let home = site::<code::Home>(&u);
    assert_eq!(home.value, Some(path("index.md")));
    assert_eq!(home.rejected.len(), 1);
    let none = Tree::new().file("a.md", "x\n").understand(&Pinned::none());
    assert!(none.site.as_ref().unwrap().home().is_none());
    assert!(none.notices.iter().any(|n| n.code() == "site.home.generated"));
    let pinned = Tree::new().file("a.md", "x\n").understand(&Pinned::none().site::<code::Home>(Some(path("b.md"))));
    assert_eq!(codes(&pinned), ["pin.target.unpublished"]);
    assert!(pinned.site.is_none());
}

#[test]
fn nav_lists_top_pages_that_are_not_posts_by_title() {
    let u = Tree::new()
        .file("index.md", "x\n")
        .file("zeta.md", "---\ntitle: About\n---\n")
        .file("alpha.md", "---\ntitle: Colophon\n---\n")
        .file("posts/p.md", "x\n")
        .file("deep/d.md", "x\n")
        .understand(&Pinned::none());
    assert_eq!(site::<code::Nav>(&u).value, vec![path("zeta.md"), path("alpha.md")]);
    assert_eq!(u.site.unwrap().nav().len(), 2);
}

#[test]
fn colliding_routes_stop_the_build_but_keep_every_decision() {
    let u = Tree::new().file("a.md", "x\n").file("A!.md", "x\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["page.route.collision"]);
    assert!(u.site.is_none());
    assert_ne!(u.decisions.len(), 0);
    let e = &u.diagnostics[0];
    assert!(matches!(&e.problem, model::Problem::PageRouteCollision { files, .. } if files.len() == 2));
}

#[test]
fn a_lost_blob_stops_the_build() {
    let u = Tree::new().file("a.md", "x\n").lost("b.md").understand(&Pinned::none());
    assert_eq!(codes(&u), ["source.blob.missing"]);
    assert!(u.site.is_none());
}

#[test]
fn a_format_problem_names_its_file() {
    let u = Tree::new().file("a.md", "---\nnope: 1\n---\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["frontmatter.syntax.invalid"]);
    assert_eq!(u.diagnostics[0].subject, Some(subject("a.md")));
    assert!(u.site.is_some());
}

#[test]
fn explain_filters_by_subject() {
    let u = Tree::new().file("a.md", "x\n").file("b.md", "x\n").understand(&Pinned::none());
    let a_md = subject("a.md");
    let a: Vec<_> = u.explain(Some(&a_md)).collect();
    assert_ne!(a.len(), 0);
    assert!(a.iter().all(|d| d.subject() == subject("a.md")));
    assert_eq!(u.explain(None).count(), u.decisions.len());
}

#[test]
fn draft_false_does_not_select_a_vault_note() {
    let tree = Tree::new().file(".obsidian/app.json", "{}").file("Private/a.md", "---\ndraft: false\n---\n");
    let u = tree.understand(&Pinned::none());
    let a = on::<code::Publish>(&u, "Private/a.md");
    assert!(!a.value);
    assert_eq!(a.evidence, Evidence::Default { reason: DefaultReason::NotSelected });
    assert_eq!(u.site.unwrap().pages(), []);
}
