//! The contents of a site as a tree: its order, its neighbours and a page's outline.

use model::{
    Anchor, Block, ContentPath, Document, Identity, Inline, Layout, Level, Order, Page, PageId, PageKind, Root, Route,
    Section, Site, SiteParts, Slug, When,
};

fn page(file: &str, route: &str, title: &str) -> Page {
    Page::new(ContentPath::parse(file).unwrap(), Route::parse(route).unwrap(), PageKind::Page, title)
}

fn dated(file: &str, route: &str, day: &str) -> Page {
    let when: When = day.parse().unwrap();
    page(file, route, file).dated(Some(when), None)
}

fn site(root: Root, pages: Vec<Page>, nav: &[&str], sections: Vec<Section>) -> Site {
    let mut parts = SiteParts::new(Identity::new("Docs", "en".parse().unwrap()), root, Layout::Flat);
    parts.nav = nav.iter().map(|file| PageId::of(&ContentPath::parse(*file).unwrap())).collect();
    parts.pages = pages;
    parts.sections = sections;
    Site::new(parts).unwrap()
}

fn files(pages: &[&Page]) -> Vec<String> {
    pages.iter().map(|page| page.file.to_string()).collect()
}

fn section(folder: &str, weight: i32, order: Order) -> Section {
    Section::new(ContentPath::parse(folder).unwrap(), weight, order)
}

#[test]
fn the_top_lists_the_navigation_first_then_the_folders_by_weight() {
    let pages = vec![
        page("a.md", "/a", "A"),
        page("b.md", "/b", "B"),
        page("c.md", "/c", "C"),
        page("zed/one.md", "/zed/one", "One"),
        page("alpha/one.md", "/alpha/one", "One"),
    ];
    let site = site(Root::Top, pages, &["c.md", "a.md"], vec![section("zed", -1, Order::Route)]);
    let top = site.contents();
    assert_eq!(files(&top.pages), ["c.md", "a.md", "b.md"]);
    let names: Vec<&str> = top.branches.iter().map(model::Branch::name).collect();
    assert_eq!(names, ["zed", "alpha"]);
}

#[test]
fn a_folder_lists_its_pages_in_the_order_its_section_says() {
    let pages = vec![
        dated("log/2024-01-01-a.md", "/log/a", "2024-01-01"),
        dated("log/2024-03-01-b.md", "/log/b", "2024-03-01"),
        page("log/c.md", "/log/c", "Undated"),
    ];
    let newest = site(Root::Top, pages.clone(), &[], vec![section("log", 0, Order::Newest)]);
    assert_eq!(files(&newest.contents().branches[0].pages), ["log/2024-03-01-b.md", "log/2024-01-01-a.md", "log/c.md"]);
    let oldest = site(Root::Top, pages.clone(), &[], vec![section("log", 0, Order::Oldest)]);
    assert_eq!(files(&oldest.contents().branches[0].pages), ["log/2024-01-01-a.md", "log/2024-03-01-b.md", "log/c.md"]);
    let title = site(Root::Top, pages, &[], vec![section("log", 0, Order::Title)]);
    assert_eq!(files(&title.contents().branches[0].pages), ["log/c.md", "log/2024-01-01-a.md", "log/2024-03-01-b.md"]);
}

#[test]
fn a_folders_index_stands_for_the_folder_and_the_home_is_not_in_the_contents() {
    let mut home = page("index.md", "/", "Home");
    home.kind = PageKind::Home;
    let pages = vec![home, page("guide/README.md", "/guide", "The guide"), page("guide/x.md", "/guide/x", "X")];
    let site = site(Root::Top, pages, &[], Vec::new());
    let top = site.contents();
    assert_eq!(top.pages.len(), 0);
    let guide = &top.branches[0];
    assert_eq!(guide.title(), "The guide");
    assert_eq!(files(&guide.pages), ["guide/x.md"]);
}

#[test]
fn a_site_that_starts_in_a_folder_names_folders_from_the_snapshot() {
    let pages = vec![page("docs/a.md", "/a", "A"), page("docs/sub/b.md", "/sub/b", "B")];
    let site =
        site(Root::Folder(ContentPath::parse("docs").unwrap()), pages, &[], vec![section("docs/sub", 0, Order::Route)]);
    let top = site.contents();
    assert_eq!(files(&top.pages), ["docs/a.md"]);
    assert_eq!(top.branches[0].folder.as_ref().unwrap().as_str(), "docs/sub");
}

#[test]
fn neighbours_follow_reading_order_across_folders() {
    let pages = vec![page("a.md", "/a", "A"), page("dir/x.md", "/dir/x", "X"), page("dir/y.md", "/dir/y", "Y")];
    let site = site(Root::Top, pages, &[], Vec::new());
    let by = |file: &str| site.pages().iter().find(|page| page.file.as_str() == file).unwrap();
    let middle = site.neighbours(by("dir/x.md"));
    assert_eq!(middle.previous.unwrap().file.as_str(), "a.md");
    assert_eq!(middle.next.unwrap().file.as_str(), "dir/y.md");
    assert!(site.neighbours(by("a.md")).previous.is_none());
    assert!(site.neighbours(by("dir/y.md")).next.is_none());
}

#[test]
fn an_outline_lists_headings_in_order_with_their_plain_text() {
    let heading = |level: u8, anchor: &str, inlines: Vec<Inline>| Block::Heading {
        level: Level::new(level).unwrap(),
        anchor: Anchor::new(Slug::parse(anchor).unwrap()),
        inlines,
    };
    let document = Document::new(vec![
        heading(1, "top", vec![Inline::Text("Top".into())]),
        Block::Paragraph(vec![Inline::Text("body".into())]),
        heading(
            2,
            "run",
            vec![
                Inline::Text("Run ".into()),
                Inline::Code("ink build".into()),
                Inline::Strong(vec![Inline::Text("now".into())]),
            ],
        ),
        Block::Quote(vec![heading(3, "deep", vec![Inline::Text("Deep".into())])]),
    ]);
    let outline = document.outline();
    let seen: Vec<(u8, &str, &str)> =
        outline.iter().map(|found| (found.level.get(), found.anchor.as_str(), found.text.as_str())).collect();
    assert_eq!(seen, [(1, "top", "Top"), (2, "run", "Run ink buildnow"), (3, "deep", "Deep")]);
}
