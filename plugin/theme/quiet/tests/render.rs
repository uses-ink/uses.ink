//! The theme through `engine::Theme`: one page holding every node of the model, compared with
//! `fixture/every-node.snap`, and the rules no page may break.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_theme_quiet as theme_quiet;

use engine::{Said, Theme};
use model::{
    Alignment, Anchor, Asset, Block, CalloutKind, Definition, Document, Footnote, FootnoteName, ImageSource, Inline,
    Item, Level, LinkTarget, Page, PageKind, Route, Site, SiteParts, Slug, Tag, Task, When,
};
use theme_quiet::Quiet;

fn text(text: &str) -> Inline {
    Inline::Text(text.to_owned())
}

fn para(inlines: Vec<Inline>) -> Block {
    Block::Paragraph(inlines)
}

fn note(name: &str) -> FootnoteName {
    FootnoteName::from(Slug::parse(name).unwrap())
}

#[expect(clippy::too_many_lines, reason = "one block of every kind, written out")]
fn body(other: &Page) -> Document {
    let heading = |level, words: &str| Block::Heading {
        level: Level::new(level).unwrap(),
        anchor: Anchor::of_heading(words).unwrap(),
        inlines: vec![text(words)],
    };
    let link = |target, words: &str| Inline::Link { target, children: vec![text(words)] };
    let blocks = vec![
        heading(2, "Every node"),
        para(vec![
            text("Plain, "),
            Inline::Emphasis(vec![text("em")]),
            Inline::Strong(vec![text("strong")]),
            Inline::Strike(vec![text("gone")]),
            Inline::Code("x < y".to_owned()),
            Inline::Highlight(vec![text("marked")]),
            Inline::Superscript(vec![text("2")]),
            Inline::Subscript(vec![text("i")]),
            Inline::Keyboard("Ctrl".to_owned()),
            Inline::SoftBreak,
            Inline::Break,
            Inline::Math("x^2".to_owned()),
            Inline::Math("\\frac{".to_owned()),
            Inline::Footnote(note("one")),
            Inline::Footnote(note("one")),
        ]),
        para(vec![
            link(LinkTarget::Page { id: other.id, anchor: Some(Anchor::of_heading("Install").unwrap()) }, "other"),
            link(LinkTarget::Anchor(Anchor::of_heading("Every node").unwrap()), "up"),
            link(LinkTarget::External("https://uses.ink".parse().unwrap()), "out"),
            link(LinkTarget::Unresolved { written: "nowhere.md".to_owned() }, "nowhere"),
            Inline::Image { source: ImageSource::Asset("posts/nib.png".parse().unwrap()), alt: "a nib".to_owned() },
            Inline::Image {
                source: ImageSource::Unresolved { written: "gone.png".to_owned() },
                alt: "lost".to_owned(),
            },
        ]),
        Block::List {
            ordered: None,
            items: vec![
                Item::task(Task::Done, vec![para(vec![text("done")])]),
                Item::task(Task::Open, vec![para(vec![text("open")])]),
            ],
            tight: true,
        },
        Block::List { ordered: Some(3), items: vec![Item::new(vec![para(vec![text("third")])])], tight: true },
        Block::Quote(vec![para(vec![text("quoted")])]),
        Block::Code { language: Some("rust".to_owned()), text: "fn main() {}\n".to_owned() },
        Block::Code { language: Some("no-such-language".to_owned()), text: "<plain>\n".to_owned() },
        Block::Code { language: None, text: "bare\n".to_owned() },
        Block::Rule,
        Block::Table {
            alignments: vec![Alignment::Left, Alignment::Right],
            head: vec![vec![text("a")], vec![text("b")]],
            rows: vec![vec![vec![text("1")], vec![text("2")]]],
        },
        Block::Callout { kind: CalloutKind::Warning, title: None, blocks: vec![para(vec![text("careful")])] },
        Block::Callout {
            kind: CalloutKind::Note,
            title: Some(vec![text("Titled")]),
            blocks: vec![para(vec![text("noted")])],
        },
        Block::Math("\\sum_i i".to_owned()),
        Block::Definitions(vec![Definition::new(vec![text("term")], vec![vec![para(vec![text("meaning")])]])]),
        Block::Details { summary: vec![text("more")], blocks: vec![para(vec![text("hidden")])] },
    ];
    Document::new(blocks).with_footnotes(vec![Footnote::new(note("one"), vec![para(vec![text("A note.")])])])
}

fn site() -> Site {
    let other = Page::new("usage.md".parse().unwrap(), "/usage".parse().unwrap(), PageKind::Page, "Usage");
    let post = Page::new("posts/a.md".parse().unwrap(), "/posts/a".parse().unwrap(), PageKind::Post, "Every node")
        .holding(body(&other))
        .dated(Some(When::Day(jiff::civil::date(2024, 3, 1))), None)
        .tagged(vec![Tag::new("ink").unwrap(), Tag::new("Pens").unwrap()])
        .summarized(Some("All of it.".to_owned()));
    let home = Page::new("index.md".parse().unwrap(), Route::root(), PageKind::Home, "Ink");
    let mut parts =
        SiteParts::new(model::Identity::new("Ink", "en".parse().unwrap()), model::Root::Top, model::Layout::Blog);
    parts.home = Some(home.id);
    parts.nav = vec![other.id];
    parts.assets =
        vec![Asset::new("posts/nib.png".parse().unwrap(), "/posts/nib.png".parse().unwrap(), model::Hash::of(b"png"))];
    parts.pages = vec![home, post, other];
    Site::new(parts).unwrap()
}

fn quiet() -> Quiet {
    Quiet::new("https://ink.uses.ink".parse().unwrap())
}

fn page<'a>(site: &'a Site, route: &str) -> &'a Page {
    site.pages().iter().find(|page| page.route.as_str() == route).unwrap()
}

#[test]
fn highlight_renders_every_node() {
    let (site, theme) = (site(), quiet());
    let mut said = Said::default();
    let html = theme.render(page(&site, "/posts/a"), &site, &mut said).unwrap();
    let codes: Vec<&str> = said
        .diagnostics
        .iter()
        .map(model::Diagnostic::code)
        .chain(said.notices.iter().map(model::Notice::code))
        .collect();
    assert_eq!(codes, ["math.syntax.invalid", "code.language.unknown"]);
    Script(html.as_ref()).refuse(&model::Route::root());
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture"));
    settings.set_prepend_module_to_snapshot(false);
    settings.set_omit_expression(true);
    settings.bind(|| insta::assert_snapshot!("every-node", html.as_ref().replace('>', ">\n")));
}

#[test]
fn the_home_lists_the_posts() {
    let (site, theme) = (site(), quiet());
    let html = theme.render(page(&site, "/"), &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    assert!(html.contains("<title>Ink</title>"));
    assert!(
        html.contains(r#"<time datetime="2024-03-01">2024-03-01</time> <a href="/posts/a">Every node</a>"#),
        "{html}"
    );
    assert!(html.contains(r#"<link rel="alternate" type="application/atom+xml" title="Ink" href="/feed.xml">"#));
}

/// What a reader page may never hold.
struct Script<'a>(&'a str);

impl Script<'_> {
    fn refuse(&self, route: &model::Route) {
        let html = self.0.to_ascii_lowercase();
        assert!(!html.contains("<script"), "{route}");
        assert!(!html.contains("javascript:"), "{route}");
        assert!(!html.contains(" style="), "{route}: inline styles break the CSP");
    }
}

/// Pages without code, so giallo is not loaded here; the page with code is checked in
/// `highlight_renders_every_node`.
#[test]
fn no_page_runs_script() {
    let (site, theme) = (site(), quiet());
    for page in site.pages().iter().filter(|page| page.kind != PageKind::Post) {
        let html = theme.render(page, &site, &mut Said::default()).unwrap();
        Script(html.as_ref()).refuse(&page.route);
    }
    let assets = theme.assets();
    let routes: Vec<&str> = assets.iter().map(|file| file.route.as_str()).collect();
    assert_eq!(routes, ["/style.css", "/code.css"]);
    let code = String::from_utf8(assets[1].bytes.clone()).unwrap();
    assert!(code.contains(".hl-l-code") && code.contains("@media (prefers-color-scheme: dark)"));
}

/// `code.css` is what giallo writes; a stale one is rewritten and the run fails, to be read.
#[test]
fn highlight_stylesheet_is_fresh() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/code.css");
    let fresh = theme_quiet::code_css().unwrap();
    let held = std::fs::read_to_string(path).unwrap_or_default();
    if held != fresh {
        std::fs::write(path, &fresh).unwrap();
        panic!("code.css was stale and is rewritten: read the diff");
    }
}
