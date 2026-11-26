//! Rendering: the same site twice is the same deploy; the engine's headers, feeds, sitemap,
//! `robots.txt` and not-found page; theme files that collide are refused.

use engine::{Engine, File, NoPins, Rendered};
use model::{HeaderName, MediaType, Route, Status};

use crate::support::{Pinned, Plain, Stand, Tree, settings};

fn blog() -> Tree {
    Tree::new()
        .file("index.md", "---\ntitle: Ink\ndescription: Notes on ink.\nlang: fr\n---\n")
        .file("about.md", "# About\n")
        .file("posts/2024-03-01-first.md", "---\ntags: ink, pens\n---\nThe first post. It is short.\n")
        .file("posts/second.md", "---\ndate: 2024-04-02T10:00:00Z\n---\n![p](p.png)\n")
        .file("posts/p.png", "png")
        .file("posts/undated.md", "---\ntype: post\n---\n")
}

fn rendered(tree: &Tree, theme: &Plain) -> Rendered {
    tree.render(theme)
}

fn route(text: &str) -> Route {
    text.parse().unwrap()
}

fn text(rendered: &Rendered, at: &str) -> String {
    let entry = rendered.manifest.entry(&route(at)).unwrap();
    String::from_utf8(rendered.blobs[&entry.blob()].clone()).unwrap()
}

#[test]
fn the_same_input_gives_the_same_deploy() {
    let theme = Plain::default();
    let first = rendered(&blog(), &theme);
    let second = rendered(&blog(), &theme);
    assert_eq!(first.manifest.id(), second.manifest.id());
    assert_eq!(first.blobs, second.blobs);
    let changed = rendered(&blog().file("more.md", "x\n"), &theme);
    assert_ne!(first.manifest.id(), changed.manifest.id());
}

#[test]
fn every_page_carries_the_engines_headers() {
    let rendered =
        rendered(&blog().file("posts/nib.svg", "<svg/>").file("posts/third.md", "![n](nib.svg)\n"), &Plain::default());
    assert!(rendered.diagnostics.is_empty(), "{:?}", rendered.diagnostics);
    let routes: Vec<&str> = rendered.manifest.entries().iter().map(|e| e.route().as_str()).collect();
    let pages: Vec<&str> = routes.iter().copied().filter(|r| !r.starts_with("/posts/p.")).collect();
    assert_eq!(
        pages,
        [
            "/",
            "/404",
            "/about",
            "/feed.xml",
            "/posts/first",
            "/posts/nib.svg",
            "/posts/second",
            "/posts/third",
            "/posts/undated",
            "/robots.txt",
            "/rss.xml",
            "/sitemap.xml"
        ]
    );
    let csp = "default-src 'none'; img-src 'self' https: data:; style-src 'self'; font-src 'self'; media-src 'self' https:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
    for entry in rendered.manifest.entries().iter().filter(|e| e.media() == MediaType::Html) {
        let found = entry.headers().iter().find(|h| h.name == HeaderName::ContentSecurityPolicy).unwrap();
        assert_eq!(found.value.as_str(), csp, "{}", entry.route());
        assert_eq!(entry.headers().len(), 7);
        assert!(entry.headers().iter().any(|h| h.name == HeaderName::CrossOriginResourcePolicy));
    }
    let image = rendered.manifest.entry(&route("/posts/nib.svg")).unwrap();
    assert_eq!(image.media(), MediaType::Svg);
    let headers: Vec<(&str, &str)> = image.headers().iter().map(|h| (h.name.name(), h.value.as_str())).collect();
    let sandboxed = [
        ("content-security-policy", "default-src 'none'; frame-ancestors 'none'; sandbox"),
        ("cross-origin-resource-policy", "same-origin"),
        ("x-content-type-options", "nosniff"),
    ];
    assert_eq!(headers, sandboxed);
    assert!(!rendered.blobs.contains_key(&image.blob()), "the snapshot holds a plain file's bytes");
}

#[test]
fn the_not_found_page_is_served_with_404() {
    let plain = rendered(&blog(), &Plain::default());
    let missing = plain.manifest.entry(&route("/404")).unwrap();
    assert_eq!(missing.status(), Status::NotFound);
    assert!(text(&plain, "/404").contains("Page not found"));
    let own = rendered(&blog().file("404.md", "---\ntitle: Lost\n---\n"), &Plain::default());
    assert_eq!(own.manifest.entry(&route("/404")).unwrap().status(), Status::NotFound);
    assert!(text(&own, "/404").contains("Lost"));
    assert!(!text(&own, "/sitemap.xml").contains("/404"));
}

#[test]
fn a_site_without_a_home_gets_a_generated_one() {
    let rendered = rendered(&Tree::new().file("notes/a.md", "# A\n").file("notes/b.md", "# B\n"), &Plain::default());
    let home = rendered.manifest.entry(&Route::root()).unwrap();
    assert_eq!(home.status(), Status::Ok);
    assert_eq!(home.media(), MediaType::Html);
    assert!(text(&rendered, "/sitemap.xml").contains("<url><loc>https://ink.uses.ink/</loc></url>"));
    let own = self::rendered(&blog(), &Plain::default());
    assert!(text(&own, "/").contains("<title>Ink</title>"), "an author's home is never replaced");
}

#[test]
fn feeds_hold_dated_posts_newest_first() {
    let rendered = rendered(&blog(), &Plain::default());
    let atom = text(&rendered, "/feed.xml");
    let second = atom.find("https://ink.uses.ink/posts/second").unwrap();
    let first = atom.find("https://ink.uses.ink/posts/first").unwrap();
    assert!(second < first);
    assert!(atom.contains("<published>2024-03-01T00:00:00Z</published>"));
    assert!(atom.contains("<subtitle>Notes on ink.</subtitle>"));
    assert!(atom.contains("xml:lang=\"fr\""));
    assert!(atom.contains("<summary>The first post. It is short.</summary>"));
    assert!(atom.contains("<category term=\"pens\" label=\"pens\"/>"));
    assert!(!atom.contains("undated"));
    let rss = text(&rendered, "/rss.xml");
    assert!(rss.contains("<pubDate>Fri, 01 Mar 2024 00:00:00 GMT</pubDate>"));
    assert!(rendered.notices.iter().any(|n| n.code() == "feed.post.undated"));
    let none = self::rendered(&Tree::new().file("a.md", "x\n"), &Plain::default());
    assert!(none.manifest.entry(&route("/feed.xml")).is_none());
    assert!(none.notices.iter().any(|n| n.code() == "feed.post.none"));
}

#[test]
fn the_sitemap_and_robots_point_at_each_other() {
    let rendered = rendered(&blog(), &Plain::default());
    let sitemap = text(&rendered, "/sitemap.xml");
    assert!(sitemap.contains("<url><loc>https://ink.uses.ink/posts/first</loc><lastmod>2024-03-01</lastmod></url>"));
    assert!(sitemap.contains("<url><loc>https://ink.uses.ink/about</loc></url>"));
    assert_eq!(
        text(&rendered, "/robots.txt"),
        "User-agent: *\nAllow: /\n\nSitemap: https://ink.uses.ink/sitemap.xml\n"
    );
}

#[test]
fn theme_files_never_take_a_route_already_served() {
    let file = |at: &str| File { route: route(at), media: MediaType::Css, bytes: b"x".to_vec() };
    let theme = Plain { files: vec![file("/about"), file("/feed.xml"), file("/style.css")], refuses: vec![] };
    let rendered = rendered(&blog(), &theme);
    let codes: Vec<&str> = rendered.diagnostics.iter().map(model::Diagnostic::code).collect();
    assert_eq!(codes, ["theme.route.collision", "theme.route.reserved"]);
    assert_eq!(rendered.manifest.entry(&route("/about")).unwrap().media(), MediaType::Html);
    assert_eq!(rendered.manifest.entry(&route("/style.css")).unwrap().media(), MediaType::Css);
}

#[test]
fn a_page_the_theme_refuses_keeps_the_themes_cause_and_what_it_said() {
    let theme = Plain { files: vec![], refuses: vec!["About".to_owned()] };
    let rendered = rendered(&blog(), &theme);
    let refused = &rendered.diagnostics[0];
    assert_eq!(refused.code(), "theme.page.refused");
    assert_eq!(refused.chain().nth(1).unwrap().code(), "html.element.stripped");
    assert!(rendered.manifest.entry(&route("/about")).is_none());
    let said: Vec<&str> = rendered.notices.iter().map(model::Notice::code).collect();
    assert!(said.contains(&"code.language.unknown"), "{said:?}");
}

#[test]
fn a_pinned_plain_file_of_unknown_kind_is_left_out() {
    let tree = Tree::new().file("a.md", "x\n").file("data.zip", "zip");
    let site = tree.understand(&Pinned::none().file::<model::code::Publish>("data.zip", true)).site.unwrap();
    let (_, store) = tree.snapshot();
    let rendered = Engine::new(&store, NoPins, (), &Plain::default()).render(&site, &settings(), &Stand);
    let codes: Vec<&str> = rendered.diagnostics.iter().map(model::Diagnostic::code).collect();
    assert_eq!(codes, ["asset.media.unknown"]);
    assert!(rendered.manifest.entry(&route("/data.zip")).is_none());
}
