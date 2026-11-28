//! Each inferrer's precedence: a pin beats frontmatter, frontmatter beats the file name, the file
//! name beats a default; and the defaults say so.

use jiff::civil::date;
use model::code::{self, AnyCode};
use model::{
    Announcement, DefaultReason, Evidence, Known, Layout, PageKind, Provenance, Root, Slug, Subject, Timestamp, When,
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
}

#[test]
fn a_trace_shows_every_rung_and_the_one_chosen() {
    use engine::{Reading, Standing, Witnessed};
    let tree = Tree::new().file("a.md", "---\ntitle: Front\n---\n# Heading\n").file("b.md", "text\n");
    let subject = Subject::File(path("a.md"));
    let u = tree.trace(&Pinned::none(), AnyCode::Title, subject.clone());
    let trace = u.trace.unwrap();
    assert_eq!((trace.code, &trace.subject), (AnyCode::Title, &subject));
    let witnesses: Vec<_> = trace.rungs.iter().map(|rung| rung.witness).collect();
    assert_eq!(witnesses, [Witnessed::Pin, Witnessed::Front, Witnessed::Heading, Witnessed::Name(Reading::Stem)]);
    let (at, chosen) = trace.chosen().unwrap();
    assert_eq!(at, 2);
    assert!(matches!(chosen.testimony.evidence(), Evidence::Frontmatter { .. }));
    let heading = &trace.rungs[2].heard;
    assert_eq!(heading.len(), 1, "the outranked heading is still heard");
    assert_eq!((heading[0].standing, heading[0].chosen), (Standing::Value, false));
    assert!(trace.rungs[0].heard.is_empty() && trace.rungs[3].heard.is_empty(), "no pin, and the name is not asked");
    assert_eq!(u.decisions, tree.understand(&Pinned::none()).decisions, "tracing changes no decision");
    let ignored = tree.trace(&Pinned::none(), AnyCode::Title, Subject::File(path("nosuch.md")));
    assert_eq!(ignored.trace, None);
}

#[test]
fn tags_join_the_frontmatters_with_the_texts_and_say_which_added() {
    use engine::{Standing, Witnessed};
    let tree = Tree::new()
        .file("both.md", "---\ntags: Ink, pens\n---\n#t pens, Rust\n")
        .file("same.md", "---\ntags: Ink\n---\n#t ink\n")
        .file("text.md", "#t Rust, rust\n")
        .file("front.md", "---\ntags: Ink\n---\ntext\n")
        .file("none.md", "text\n")
        .file("pinned.md", "#t Rust\n");
    let none = tree.understand(&Pinned::none());
    let tags = |path| {
        let decision = on::<code::Tags>(&none, path);
        (decision.value.iter().map(|tag| tag.written.clone()).collect::<Vec<_>>(), decision.evidence)
    };
    let (both, evidence) = tags("both.md");
    assert_eq!(both, ["Ink", "pens", "Rust"]);
    assert!(matches!(evidence, Evidence::Inline { .. }), "the text added Rust");
    let (same, evidence) = tags("same.md");
    assert_eq!(same, ["Ink"]);
    assert!(matches!(evidence, Evidence::Frontmatter { .. }), "the text added nothing");
    let (text, evidence) = tags("text.md");
    assert_eq!(text, ["Rust"]);
    assert!(matches!(evidence, Evidence::Inline { .. }));
    assert!(matches!(tags("front.md").1, Evidence::Frontmatter { .. }));
    assert_eq!(tags("none.md"), (Vec::<String>::new(), Evidence::Default { reason: DefaultReason::NothingMatched }));
    let pinned = tree.understand(&Pinned::none().file::<code::Tags>("pinned.md", Vec::new()));
    let pin = on::<code::Tags>(&pinned, "pinned.md");
    assert!(pin.value.is_empty() && matches!(pin.evidence, Evidence::Config { .. }), "a pin states every tag");
    let traced = tree.trace(&Pinned::none(), AnyCode::Tags, Subject::File(path("both.md"))).trace.unwrap();
    let witnesses: Vec<_> = traced.rungs.iter().map(|rung| rung.witness).collect();
    assert_eq!(witnesses, [Witnessed::Pin, Witnessed::Inline, Witnessed::Front, Witnessed::Default]);
    assert_eq!(traced.chosen().map(|(at, _)| at), Some(2));
    let outranked = &traced.rungs[2].heard;
    assert_eq!((outranked[0].standing, outranked[0].chosen), (Standing::Value, false));
}

#[test]
fn the_format_is_a_pin_then_a_vault_then_markdown() {
    use engine::{Chosen, Witnessed};
    use model::record::PluginId;
    let vault = Tree::new().file(".obsidian/app.json", "{}").file("a.md", "text\n");
    let plain = Tree::new().file("a.md", "text\n").file("notes/.obsidian/x", "x");
    let chosen = |tree: &Tree, pins: &Pinned| {
        let (snapshot, _) = tree.snapshot();
        let asked = Some((AnyCode::Format, Subject::Site));
        let chosen = Chosen::new(&snapshot, pins, asked);
        let understood = tree.understand(pins).with(chosen);
        (site::<code::Format>(&understood), understood.notices, understood.trace.unwrap())
    };
    let (decision, notices, trace) = chosen(&vault, &Pinned::none());
    assert_eq!(decision.value, PluginId::obsidian());
    assert_eq!(decision.evidence, Evidence::Marker { path: path(".obsidian") });
    assert_eq!(notices[0].announcement, Announcement::FormatObsidianChosen);
    let witnesses: Vec<_> = trace.rungs.iter().map(|rung| rung.witness).collect();
    assert_eq!(witnesses, [Witnessed::Pin, Witnessed::Folder(engine::Marker::Obsidian), Witnessed::Default]);
    let (decision, notices, _) = chosen(&plain, &Pinned::none());
    assert_eq!(decision.value, PluginId::markdown());
    assert_eq!(decision.evidence, Evidence::Default { reason: DefaultReason::NothingMatched });
    assert!(notices.iter().all(|notice| notice.code() != "format.obsidian.chosen"), "markdown is the mundane default");
    let pin = |id: PluginId| Pinned::none().site::<code::Format>(id);
    let (decision, notices, trace) = chosen(&vault, &pin(PluginId::markdown()));
    assert_eq!(decision.value, PluginId::markdown());
    assert!(matches!(decision.evidence, Evidence::Config { .. }));
    assert_eq!(notices[0].announcement, Announcement::FormatPinChosen { format: "ink:format-markdown".to_owned() });
    assert_eq!(trace.chosen().map(|(at, _)| at), Some(1));
    assert_eq!(trace.rungs[1].heard.len(), 1, "the vault is heard and outranked");
    let (decision, notices, _) = chosen(&plain, &pin(PluginId::obsidian()));
    assert_eq!(decision.value, PluginId::obsidian());
    assert!(matches!(notices[0].announcement, Announcement::FormatPinChosen { .. }));
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
}

#[test]
fn a_false_date_decides_the_page_undated_over_its_file_name() {
    let tree = Tree::new()
        .file("posts/2024-03-01-x.md", "---\ndate: false\ndescription: false\n---\nFirst paragraph.\n")
        .file("posts/2024-03-02-y.md", "First paragraph.\n");
    let u = tree.understand(&Pinned::none());
    assert_eq!(codes(&u), Vec::<&str>::new(), "{:?}", u.diagnostics);
    let undated = on::<code::Date>(&u, "posts/2024-03-01-x.md");
    assert_eq!(undated.value, None);
    assert!(matches!(undated.evidence, Evidence::Frontmatter { .. }), "{:?}", undated.evidence);
    assert_eq!(on::<code::Summary>(&u, "posts/2024-03-01-x.md").value, None);
    assert_eq!(on::<code::Date>(&u, "posts/2024-03-02-y.md").value, Some(When::Day(date(2024, 3, 2))));
    assert_eq!(on::<code::Summary>(&u, "posts/2024-03-02-y.md").value.as_deref(), Some("First paragraph."));
}

#[test]
fn a_bound_is_never_a_date() {
    let bound = Known::NotAfter { value: instant("2023-05-06T07:08:09Z"), from: Provenance::Filesystem };
    let u = Tree::new().file("posts/c.md", "x\n").created("posts/c.md", bound).understand(&Pinned::none());
    let c = on::<code::Date>(&u, "posts/c.md");
    assert_eq!(c.value, None);
    assert_eq!(c.evidence, Evidence::Default { reason: DefaultReason::OnlyBound });
    let bound = Announcement::FactDateBound { decision: AnyCode::Date, bound: "2023-05-06T07:08:09Z".to_owned() };
    assert!(u.notices.iter().any(|n| n.announcement == bound), "{:?}", u.notices);
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
    // a plain page is the mundane default, not worth a notice on every page
    assert!(!u.notices.iter().any(|n| n.code() == "notice.default.chosen" && n.subject == Some(subject("d.md"))));
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

#[test]
fn folder_evidence_names_the_folder() {
    let u = Tree::new()
        .file(".obsidian/app.json", "{}")
        .file("content/posts/a.md", "---\npublish: true\n---\n")
        .file("content/notes/drafts/b.md", "x\n")
        .file("content/_c.md", "x\n")
        .file("content/node_modules/x/y.md", "x\n")
        .understand(&Pinned::none().site::<code::Root>(Root::Folder(path("content"))));
    let folder = |p: &str| Evidence::Marker { path: path(p) };
    assert_eq!(on::<code::Kind>(&u, "content/posts/a.md").evidence, folder("content/posts"));
    assert_eq!(on::<code::Publish>(&u, "content/notes/drafts/b.md").evidence, folder("content/notes/drafts"));
    assert_eq!(on::<code::Publish>(&u, "content/_c.md").evidence, Evidence::Filename { path: path("content/_c.md") });
    assert_eq!(on::<code::Ignored>(&u, "content/node_modules/x/y.md").evidence, folder("content/node_modules"));
    let vault = Tree::new().file(".obsidian/app.json", "{}").file("a.md", "x\n").understand(&Pinned::none());
    assert_eq!(site::<code::Layout>(&vault).evidence, folder(".obsidian"));
    let blog = Tree::new().file("posts/a.md", "x\n").understand(&Pinned::none());
    assert_eq!(site::<code::Layout>(&blog).evidence, folder("posts"));
}

#[test]
fn repository_files_match_their_whole_name_and_say_they_are_ignored() {
    let u = Tree::new()
        .file("LICENSE", "x\n")
        .file("LICENSE-MIT", "x\n")
        .file("LICENCE.md", "x\n")
        .file("Security.md", "x\n")
        .file("CHANGELOG.md", "x\n")
        .file("security-checklist.md", "x\n")
        .file("license-guide.md", "x\n")
        .file("changelogs.md", "x\n")
        .file(".github/a.md", "x\n")
        .file(".github/b.md", "x\n")
        .file("notes/private.md", "x\n")
        .understand(&Pinned::none().exclude("notes/"));
    let ignored = |p| on::<code::Ignored>(&u, p).value;
    for p in ["LICENSE", "LICENSE-MIT", "LICENCE.md", "Security.md", "CHANGELOG.md", ".github/a.md"] {
        assert!(ignored(p), "{p}");
    }
    for p in ["security-checklist.md", "license-guide.md", "changelogs.md"] {
        assert!(!ignored(p), "{p}");
    }
    let said: Vec<String> = u
        .notices
        .iter()
        .filter_map(|n| match &n.announcement {
            model::Announcement::FileIgnoreInferred { path } => Some(path.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(said, [".github", "CHANGELOG.md", "LICENCE.md", "LICENSE", "LICENSE-MIT", "Security.md"]);
}

#[test]
fn a_posts_folders_index_is_a_listing_page() {
    let u = Tree::new()
        .file("posts/index.md", "---\ndate: 2024-03-01\n---\n")
        .file("blog/README.md", "x\n")
        .file("posts/a.md", "x\n")
        .file("posts/bundle/index.md", "x\n")
        .understand(&Pinned::none());
    let kind = |p| on::<code::Kind>(&u, p);
    assert_eq!(kind("posts/index.md").value, PageKind::Page);
    assert_eq!(kind("posts/index.md").evidence, Evidence::Marker { path: path("posts") });
    assert_eq!(kind("blog/README.md").value, PageKind::Page);
    assert_eq!(kind("posts/a.md").value, PageKind::Post);
    assert_eq!(kind("posts/bundle/index.md").value, PageKind::Post);
}

#[test]
fn a_vaults_templates_folder_is_ignored() {
    let vault = Tree::new()
        .file(".obsidian/app.json", "{}")
        .file("Templates/daily.md", "---\npublish: true\n---\n")
        .file("notes/templates.md", "---\npublish: true\n---\n")
        .understand(&Pinned::none());
    let t = on::<code::Ignored>(&vault, "Templates/daily.md");
    assert!(t.value);
    assert_eq!(t.evidence, Evidence::Marker { path: path("Templates") });
    assert!(!on::<code::Ignored>(&vault, "notes/templates.md").value);
    assert!(
        vault
            .notices
            .iter()
            .any(|n| n.announcement == model::Announcement::FileIgnoreInferred { path: path("Templates") })
    );
    let blog = Tree::new().file("templates/a.md", "x\n").understand(&Pinned::none());
    assert!(!on::<code::Ignored>(&blog, "templates/a.md").value);
}

#[test]
fn a_vaults_templates_folder_is_the_one_templates_json_names() {
    let vault = Tree::new()
        .file(".obsidian/app.json", "{}")
        .file(".obsidian/templates.json", r#"{"folder": "Meta/Tpl"}"#)
        .file("meta/tpl/daily.md", "x\n")
        .file("Templates/kept.md", "x\n")
        .file("Meta/a.md", "x\n")
        .understand(&Pinned::none());
    let t = on::<code::Ignored>(&vault, "meta/tpl/daily.md");
    assert!(t.value);
    assert_eq!(t.evidence, Evidence::Marker { path: path("meta/tpl") });
    // the conventional name is no templates folder once another is set
    assert!(!on::<code::Ignored>(&vault, "Templates/kept.md").value);
    assert!(!on::<code::Ignored>(&vault, "Meta/a.md").value);
    assert!(vault.diagnostics.is_empty(), "{:?}", vault.diagnostics);
}

#[test]
fn a_templates_folder_of_a_plugin_that_is_off_is_not_used() {
    let off = Tree::new()
        .file(".obsidian/app.json", "{}")
        .file(".obsidian/templates.json", r#"{"folder": "Tpl"}"#)
        .file(".obsidian/core-plugins.json", r#"["file-explorer"]"#)
        .file("Tpl/a.md", "x\n")
        .file("templates/b.md", "x\n")
        .understand(&Pinned::none());
    assert!(!on::<code::Ignored>(&off, "Tpl/a.md").value);
    assert!(on::<code::Ignored>(&off, "templates/b.md").value);
    let on_ = Tree::new()
        .file(".obsidian/app.json", "{}")
        .file(".obsidian/templates.json", r#"{"folder": "Tpl"}"#)
        .file(".obsidian/core-plugins.json", r#"{"templates": true}"#)
        .file("Tpl/a.md", "x\n")
        .understand(&Pinned::none());
    assert!(on::<code::Ignored>(&on_, "Tpl/a.md").value);
}

#[test]
fn templates_settings_that_do_not_read_are_said_and_the_convention_stands() {
    let vault = Tree::new()
        .file(".obsidian/app.json", "{}")
        .file(".obsidian/templates.json", r#"{"folder": "../out"}"#)
        .file(".obsidian/core-plugins.json", "not json")
        .file("templates/a.md", "x\n")
        .understand(&Pinned::none());
    let invalid: Vec<_> = vault
        .diagnostics
        .iter()
        .filter_map(|d| match &d.problem {
            model::Problem::VaultSettingsInvalid { path, .. } => Some(path.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(invalid, [".obsidian/templates.json", ".obsidian/core-plugins.json"]);
    assert!(on::<code::Ignored>(&vault, "templates/a.md").value);
}

#[test]
fn a_folder_table_makes_the_dated_pages_in_it_pages_unless_a_page_says_otherwise() {
    use engine::Witnessed;
    let tree = Tree::new()
        .file("log/a.md", "---\ndate: 2024-03-01\n---\ntext\n")
        .file("log/b.md", "---\ndate: 2024-03-02\ntype: post\n---\ntext\n")
        .file("c.md", "---\ndate: 2024-03-03\n---\ntext\n");
    let kind = |understood, file| on::<code::Kind>(understood, file);
    let plain = tree.understand(&Pinned::none());
    assert_eq!(kind(&plain, "log/a.md").value, PageKind::Post, "a date alone makes a post");
    let ruled = tree.understand(&Pinned::none().folder("log", PageKind::Page));
    let a = kind(&ruled, "log/a.md");
    assert_eq!((a.value, matches!(a.evidence, Evidence::Config { .. })), (PageKind::Page, true));
    assert_eq!(kind(&ruled, "log/b.md").value, PageKind::Post, "a page's own type outranks the folder");
    assert_eq!(kind(&ruled, "c.md").value, PageKind::Post, "a page outside the folder is not ruled");
    let traced = tree.trace(&Pinned::none().folder("log", PageKind::Page), AnyCode::Kind, subject("log/a.md"));
    let witnesses: Vec<_> = traced.trace.unwrap().rungs.iter().map(|rung| rung.witness).collect();
    assert_eq!(witnesses[..3], [Witnessed::Pin, Witnessed::Front, Witnessed::Rule]);
}

#[test]
fn the_sections_of_the_pins_reach_the_site() {
    use model::{Order, Section};
    let tree = Tree::new().file("a/x.md", "text\n").file("b/y.md", "text\n");
    let section = Section::new(path("b"), -2, Order::Title);
    let understood = tree.understand(&Pinned::none().section(section.clone()));
    assert_eq!(understood.site.unwrap().sections(), [section]);
}
