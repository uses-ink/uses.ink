//! The theme through `engine::Theme`: one page holding every node of the model, compared with
//! `fixture/every-node.snap`, and the rules no page may break. Code goes through a stand-in
//! highlighter; the real grammars are host-native's and its tests'.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_theme_quiet as theme_quiet;

use std::convert::Infallible;

use engine::{Said, Theme};
use model::highlight::{Capture, Highlight, Run, Unhighlighted};
use model::{
    Alignment, Anchor, Asset, Block, CalloutKind, Definition, Document, Folding, Footnote, FootnoteName, ImageSource,
    Inline, Item, Level, LinkTarget, Page, PageKind, Route, Site, SiteParts, Slug, Tag, Task, When,
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
        Block::Callout {
            kind: CalloutKind::Warning,
            title: None,
            blocks: vec![para(vec![text("careful")])],
            folding: Folding::Fixed,
        },
        Block::Callout {
            kind: CalloutKind::Note,
            title: Some(vec![text("Titled")]),
            blocks: vec![para(vec![text("noted")])],
            folding: Folding::Fixed,
        },
        Block::Callout {
            kind: CalloutKind::Tip,
            title: Some(vec![text("Folded")]),
            blocks: vec![para(vec![text("hidden at first")])],
            folding: Folding::Closed,
        },
        Block::Callout {
            kind: CalloutKind::Note,
            title: None,
            blocks: vec![para(vec![text("open")])],
            folding: Folding::Open,
        },
        para(vec![text("A block with an id."), Inline::Anchor(Anchor::new(Slug::parse("block").unwrap()))]),
        Block::Embed {
            target: LinkTarget::Unresolved { written: "Other".to_owned() },
            blocks: vec![para(vec![text("embedded")])],
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

/// Knows `rust`, in which the first word is a keyword and the rest is plain.
#[derive(Debug)]
struct Stand;

impl Highlight for Stand {
    type Error = Infallible;

    fn highlight<'c>(&self, language: &str, code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<Infallible>> {
        if language != "rust" {
            return Err(Unhighlighted::Unknown);
        }
        let (word, rest) = code.split_at(code.find(' ').unwrap_or(code.len()));
        Ok(vec![Run { capture: Some(Capture::Keyword), text: word }, Run { capture: None, text: rest }])
    }
}

fn quiet() -> Quiet<Stand> {
    Quiet::new("https://ink.uses.ink".parse().unwrap(), Stand)
}

fn page<'a>(site: &'a Site, route: &str) -> &'a Page {
    site.pages().iter().find(|page| page.route.as_str() == route).unwrap()
}

#[test]
fn the_theme_renders_every_node() {
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

#[test]
fn no_page_runs_script() {
    let (site, theme) = (site(), quiet());
    for page in site.pages() {
        let html = theme.render(page, &site, &mut Said::default()).unwrap();
        Script(html.as_ref()).refuse(&page.route);
    }
    let assets = theme.assets();
    let routes: Vec<&str> = assets.iter().map(|file| file.route.as_str()).collect();
    assert_eq!(routes, ["/style.css", "/code.css"]);
}

/// Every capture has a class of its own in the code stylesheet, each styled there.
#[test]
fn every_capture_is_styled() {
    let theme = quiet();
    let code = String::from_utf8(theme.assets()[1].bytes.clone()).unwrap();
    let mut classes: Vec<String> = Capture::ALL
        .iter()
        .map(|capture| {
            let run = [Run { capture: Some(*capture), text: "x" }];
            Shown(&run).class()
        })
        .collect();
    for class in &classes {
        assert!(code.contains(&format!(".{class}")), "{class} is not in the code stylesheet");
    }
    classes.sort();
    classes.dedup();
    assert_eq!(classes.len(), Capture::ALL.len(), "two captures share a class");
    assert!(code.contains("@media (prefers-color-scheme:dark)"), "{code}");
}

/// A page whose one block is code highlighted as `runs`, so its class can be read back.
struct Shown<'r>(&'r [Run<'r>]);

impl Shown<'_> {
    fn class(&self) -> String {
        let runs = self.0.to_vec();
        let theme = Quiet::new("https://ink.uses.ink".parse().unwrap(), Fixed(runs));
        let code = Block::Code { language: Some("any".to_owned()), text: "x".to_owned() };
        let page = Page::new("a.md".parse().unwrap(), "/a".parse().unwrap(), PageKind::Page, "A")
            .holding(Document::new(vec![code]));
        let mut parts =
            SiteParts::new(model::Identity::new("Ink", "en".parse().unwrap()), model::Root::Top, model::Layout::Blog);
        parts.pages = vec![page];
        let site = Site::new(parts).unwrap();
        let html = theme.render(&site.pages()[0], &site, &mut Said::default()).unwrap();
        let html = html.as_ref();
        let at = html.find("<span class=\"").unwrap() + "<span class=\"".len();
        html[at..].split('"').next().unwrap().to_owned()
    }
}

/// Highlights anything as the same runs.
#[derive(Debug)]
struct Fixed<'r>(Vec<Run<'r>>);

impl Highlight for Fixed<'_> {
    type Error = Infallible;

    fn highlight<'c>(&self, _language: &str, _code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<Infallible>> {
        Ok(self.0.iter().map(|run| Run { capture: run.capture, text: "x" }).collect())
    }
}

#[test]
fn math_holds_only_mathml() {
    let hostile = [r"\htmlClass{x}{y}", r"\href{javascript:alert(1)}{x}", r"\text{<img src=x onerror=alert(1)>}"];
    let body = Document::new(vec![para(hostile.iter().map(|tex| Inline::Math((*tex).to_owned())).collect())]);
    let page = Page::new("a.md".parse().unwrap(), "/a".parse().unwrap(), PageKind::Page, "Math").holding(body);
    let mut parts =
        SiteParts::new(model::Identity::new("Ink", "en".parse().unwrap()), model::Root::Top, model::Layout::Flat);
    parts.pages = vec![page];
    let site = Site::new(parts).unwrap();
    let mut said = Said::default();
    let html = quiet().render(&site.pages()[0], &site, &mut said).unwrap();
    let html = html.as_ref();
    for refused in ["<img", "href=\"javascript", "class=\"x\""] {
        assert!(!html.contains(refused), "{refused} in {html}");
    }
    assert!(html.contains("<mtext>&lt;img"), "{html}");
    let codes: Vec<&str> = said.diagnostics.iter().map(model::Diagnostic::code).collect();
    assert_eq!(codes, ["math.syntax.invalid", "math.syntax.invalid"]);
}

/// A page showing `photo.jpg` and `anim.gif`, prepared as the engine would.
fn pictured() -> String {
    let route = |text: &str| text.parse::<Route>().unwrap();
    let dimensions = model::Dimensions::new(1600, 1200);
    let variant =
        |media, width, at: &str| model::Variant::new(media, model::Dimensions::new(width, width * 3 / 4), route(at));
    let photo = model::Picture::new(
        dimensions,
        vec![
            variant(model::MediaType::Avif, 960, "/photo.960.b.avif"),
            variant(model::MediaType::Jpeg, 480, "/photo.480.c.jpeg"),
            variant(model::MediaType::Avif, 480, "/photo.480.a.avif"),
            variant(model::MediaType::Jpeg, 960, "/photo.960.d.jpeg"),
        ],
        Some(route("/photo.placeholder.e.png")),
    );
    let kept = model::Picture::new(model::Dimensions::new(16, 16), Vec::new(), None);
    let image =
        |file: &str| Inline::Image { source: ImageSource::Asset(file.parse().unwrap()), alt: "a <photo>".to_owned() };
    let body = Document::new(vec![para(vec![image("photo.jpg"), image("anim.gif")])]);
    let page = Page::new("a.md".parse().unwrap(), "/a".parse().unwrap(), PageKind::Page, "A").holding(body);
    let mut parts =
        SiteParts::new(model::Identity::new("Ink", "en".parse().unwrap()), model::Root::Top, model::Layout::Flat);
    parts.pages = vec![page];
    parts.assets = vec![
        Asset::new("photo.jpg".parse().unwrap(), route("/photo.jpg"), model::Hash::of(b"photo")),
        Asset::new("anim.gif".parse().unwrap(), route("/anim.gif"), model::Hash::of(b"anim")),
    ];
    let site = Site::new(parts).unwrap().pictured(|asset| match asset.file.as_str() {
        "photo.jpg" => Some(photo.clone()),
        _ => Some(kept.clone()),
    });
    let html = quiet().render(&site.pages()[0], &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    let start = html.find("<p>").unwrap();
    html[start..html[start..].find("</p>").unwrap() + start + 4].to_owned()
}

#[test]
fn a_prepared_image_is_a_picture_with_its_size_and_placeholder() {
    let expected = concat!(
        r#"<p><span class="quiet-picture"><img src="/photo.placeholder.e.png" alt="" width="1600" height="1200">"#,
        r#"<picture><source type="image/avif" srcset="/photo.480.a.avif 480w, /photo.960.b.avif 960w" sizes="(min-width: 42rem) 40rem, calc(100vw - 2rem)">"#,
        r#"<img src="/photo.960.d.jpeg" srcset="/photo.480.c.jpeg 480w, /photo.960.d.jpeg 960w" sizes="(min-width: 42rem) 40rem, calc(100vw - 2rem)" width="1600" height="1200" alt="a &lt;photo&gt;" loading="lazy" decoding="async"></picture></span>"#,
        r#"<img src="/anim.gif" alt="a &lt;photo&gt;" width="16" height="16" loading="lazy" decoding="async"></p>"#,
    );
    assert_eq!(pictured(), expected);
}

/// A documentation site: two pages at the top and a folder of three, the folder's `README` among
/// them. The page `guide/x.md` has three headings.
fn docs() -> Site {
    let heading = |level, words: &str| Block::Heading {
        level: Level::new(level).unwrap(),
        anchor: Anchor::of_heading(words).unwrap(),
        inlines: vec![text(words)],
    };
    let page = |file: &str, route: &str, title: &str| {
        Page::new(file.parse().unwrap(), route.parse().unwrap(), PageKind::Page, title)
    };
    let long = page("guide/x.md", "/guide/x", "X").holding(Document::new(vec![
        heading(2, "Run it"),
        para(vec![text("text")]),
        heading(3, "Flags"),
        heading(2, "Read it"),
    ]));
    let home = Page::new("index.md".parse().unwrap(), Route::root(), PageKind::Home, "Docs");
    let mut parts =
        SiteParts::new(model::Identity::new("Docs", "en".parse().unwrap()), model::Root::Top, model::Layout::Docs);
    parts.home = Some(home.id);
    parts.pages = vec![
        home,
        page("a.md", "/a", "Alpha"),
        page("b.md", "/b", "Beta"),
        page("guide/README.md", "/guide", "The guide"),
        long,
        page("guide/y.md", "/guide/y", "Y"),
        page("other/z.md", "/other/z", "Z"),
    ];
    Site::new(parts).unwrap()
}

fn docs_theme() -> Quiet<Stand> {
    quiet().tokened(theme_quiet::Tokens::read([("navigation", "docs")]).unwrap())
}

#[test]
fn the_docs_navigation_draws_the_folders_the_neighbours_and_the_outline() {
    let (site, theme) = (docs(), docs_theme());
    let html = theme.render(page(&site, "/guide/x"), &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    for landmark in [
        "aria-label=\"Site\"",
        "aria-label=\"Contents\"",
        "aria-label=\"On this page\"",
        "aria-label=\"Previous and next\"",
    ] {
        assert_eq!(
            html.matches(landmark).count(),
            usize::from(landmark != "aria-label=\"Site\""),
            "{landmark} in {html}"
        );
    }
    assert!(html.contains(r#"aria-current="page">X</a>"#), "the current page is marked: {html}");
    assert_eq!(html.matches("aria-current").count(), 1, "only the current page is: {html}");
    assert!(
        html.contains("<details open><summary>The guide</summary>"),
        "the folder that holds the page is open: {html}"
    );
    assert!(html.contains("<details><summary>other</summary>"), "another folder is closed: {html}");
    assert!(
        html.contains(r##"<a href="#run-it">Run it</a>"##) && html.contains(r##"<a href="#read-it">Read it</a>"##),
        "{html}"
    );
    assert!(html.contains("Previous</span>The guide</a>") && html.contains("Next</span>Y</a>"), "{html}");
}

#[test]
fn a_page_without_two_subheadings_has_no_outline_and_the_first_page_no_previous_link() {
    let (site, theme) = (docs(), docs_theme());
    let html = theme.render(page(&site, "/a"), &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    assert!(!html.contains("On this page"), "{html}");
    assert!(html.contains("Next</span>Beta</a>") && !html.contains("Previous</span>"), "{html}");
}

#[test]
fn without_the_docs_navigation_a_page_has_the_header_alone() {
    let site = docs();
    let html = quiet().render(page(&site, "/guide/x"), &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    for drawn in ["Contents", "On this page", "Previous and next", "<details", "quiet-wide"] {
        assert!(!html.contains(drawn), "{drawn} in {html}");
    }
    assert!(!html.contains("/look.css"), "{html}");
}

#[test]
fn the_docs_navigation_reads_every_front_and_the_header_alone_reads_what_it_links() {
    let site = docs();
    let (header, docs) = (quiet(), docs_theme());
    let (inner, home) = (page(&site, "/guide/x"), page(&site, "/"));
    assert_eq!((header.reads(inner), header.reads(home)), (engine::Reads::Linked, engine::Reads::Listed));
    assert_eq!((docs.reads(inner), docs.reads(home)), (engine::Reads::Listed, engine::Reads::Listed));
}

#[test]
fn a_look_adds_its_stylesheet_and_changes_the_identity() {
    let tokens = |pairs: &[(&'static str, &'static str)]| theme_quiet::Tokens::read(pairs.iter().copied()).unwrap();
    let plain = quiet();
    let worn = quiet().tokened(tokens(&[("look", "uses-ink")]));
    let routes: Vec<String> = worn.assets().iter().map(|file| file.route.to_string()).collect();
    assert_eq!(routes[..3], ["/style.css", "/code.css", "/look.css"]);
    let look = String::from_utf8(worn.assets()[2].bytes.clone()).unwrap();
    assert!(look.contains("--paper: #ffffff") && !look.contains("prefers-color-scheme"), "{look}");
    let site = docs();
    let html = worn.render(page(&site, "/a"), &site, &mut Said::default()).unwrap();
    assert!(html.as_ref().contains(r#"<link rel="stylesheet" href="/look.css">"#));
    assert_ne!(plain.identity(), worn.identity());
    assert_ne!(plain.identity(), quiet().tokened(tokens(&[("navigation", "docs")])).identity());
}

#[test]
fn a_token_the_theme_does_not_declare_or_a_word_it_does_not_take_is_refused_with_the_words_it_has() {
    use theme_quiet::{Refused, Tokens};
    let unknown = Tokens::read([("colour", "red")]).unwrap_err();
    assert_eq!(unknown, Refused::Unknown { token: "colour".to_owned(), known: vec!["look", "navigation", "layout"] });
    assert_eq!(unknown.problem().code(), "theme.token.unknown");
    let invalid = Tokens::read([("look", "loud")]).unwrap_err();
    assert_eq!(
        invalid,
        Refused::Invalid { token: "look", value: "loud".to_owned(), accepted: vec!["quiet", "uses-ink"] }
    );
    assert_eq!(invalid.problem().code(), "theme.token.invalid");
    let declared: Vec<_> = Tokens::declared().iter().map(|token| (token.name, token.default)).collect();
    assert_eq!(declared, [("look", "quiet"), ("navigation", "header"), ("layout", "page")]);
}

#[test]
fn a_generated_home_lists_the_pages_as_the_folders_do() {
    let mut site = docs();
    let mut parts = SiteParts::new(site.identity().clone(), model::Root::Top, model::Layout::Docs);
    parts.pages = site.pages().iter().filter(|page| page.kind != PageKind::Home).cloned().collect();
    parts.nav = vec![page(&site, "/b").id];
    site = Site::new(parts).unwrap();
    let generated = Page::new("index".parse().unwrap(), Route::root(), PageKind::Home, "Docs");
    let html = quiet().render(&generated, &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    let order: Vec<usize> =
        ["Beta", "Alpha", "The guide"].iter().map(|title| html.find(&format!(">{title}</a>")).unwrap()).collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "the navigation's page first, then the rest, then the folders: {html}"
    );
}

/// A page of panels: a hero with the action and the pen's note, a rule, a panel, a rule and a foot.
fn panels() -> Site {
    let words = |level, words: &str| Block::Heading {
        level: Level::new(level).unwrap(),
        anchor: Anchor::of_heading(words).unwrap(),
        inlines: vec![text(words)],
    };
    let blocks = vec![
        words(1, "your notes"),
        para(vec![text("a lede")]),
        para(vec![
            Inline::Code("github.com/you/notes".to_owned()),
            text(" "),
            Inline::Link {
                target: LinkTarget::External("https://uses.ink/start".parse().unwrap()),
                children: vec![text("publish it")],
            },
        ]),
        para(vec![Inline::Emphasis(vec![text("no config file")])]),
        Block::Rule,
        words(2, "second"),
        para(vec![text("an ordinary paragraph with "), Inline::Emphasis(vec![text("some")]), text(" emphasis")]),
        Block::Rule,
        para(vec![text("the foot")]),
    ];
    let home =
        Page::new("index.md".parse().unwrap(), Route::root(), PageKind::Home, "Landing").holding(Document::new(blocks));
    let mut parts =
        SiteParts::new(model::Identity::new("Landing", "en".parse().unwrap()), model::Root::Top, model::Layout::Flat);
    parts.home = Some(home.id);
    parts.pages = vec![home];
    Site::new(parts).unwrap()
}

fn drawn(layout: &'static str) -> Quiet<Stand> {
    quiet().tokened(theme_quiet::Tokens::read([("look", "uses-ink"), ("layout", layout)]).unwrap())
}

#[test]
fn a_page_of_panels_is_cut_at_its_rules_and_speaks_in_the_pen_and_the_one_action() {
    let site = panels();
    let html = drawn("panels").render(page(&site, "/"), &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    assert_eq!(html.matches("<section").count(), 2, "{html}");
    assert!(html.contains("hand-hero"), "the first panel is the hero: {html}");
    assert!(html.contains(r#"<footer class="hand-end"><p>the foot</p></footer>"#), "the last is the foot: {html}");
    assert!(!html.contains("<hr"), "a rule cuts a panel and is not drawn: {html}");
    assert!(html.contains(r#"<form class="hand-ask" action="https://uses.ink/start" method="get">"#), "{html}");
    assert!(html.contains(r#"placeholder="github.com/you/notes""#) && html.contains("publish it</button>"), "{html}");
    assert!(html.contains(r#"<p class="hand-say">no config file</p>"#), "{html}");
    assert!(html.contains("an ordinary paragraph with <em>some</em> emphasis"), "{html}");
    assert!(html.contains(r#"<img class="hand-logo" src="/marks/logo-open.svg""#), "{html}");
    assert!(!html.contains("quiet-foot"), "a page of panels has no footer of the theme's: {html}");
}

#[test]
fn without_panels_the_same_blocks_are_text_and_a_rule() {
    let site = panels();
    let html = drawn("page").render(page(&site, "/"), &site, &mut Said::default()).unwrap();
    let html = html.as_ref();
    for panel in ["<section", "hand-ask", "hand-say", "hand-end", "hand-logo"] {
        assert!(!html.contains(panel), "{panel} in {html}");
    }
    assert!(html.contains("<hr>"), "{html}");
    assert!(html.contains("made with uses.ink"), "the drawn look signs the page: {html}");
}

#[test]
fn the_drawn_look_serves_its_sheet_marks_and_faces_and_the_default_look_serves_none() {
    let routes = |tokens: &theme_quiet::Tokens| -> Vec<String> {
        Quiet::<Stand>::files(tokens).iter().map(|file| file.route.to_string()).collect()
    };
    let plain = routes(&theme_quiet::Tokens::default());
    assert_eq!(plain, ["/style.css", "/code.css"]);
    let hand = routes(&theme_quiet::Tokens::read([("look", "uses-ink")]).unwrap());
    for served in ["/look.css", "/notebook.css", "/marks/frame.svg", "/marks/rule.svg", "/fonts/gaegu-bold.woff2"] {
        assert!(hand.iter().any(|route| route == served), "{served} in {hand:?}");
    }
    let site = panels();
    let html = drawn("page").render(page(&site, "/"), &site, &mut Said::default()).unwrap();
    assert!(html.as_ref().contains(r#"<link rel="stylesheet" href="/notebook.css">"#), "{}", html.as_ref());
}
