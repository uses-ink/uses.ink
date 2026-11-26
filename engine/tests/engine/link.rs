//! Link resolution: relative paths, anchors, ambiguity, missing and unpublished targets, and
//! images that make assets.

use model::code;
use model::{Block, ImageSource, Inline, LinkTarget, PageId, Problem};

use crate::support::{Pinned, Tree, codes, on, path};

/// The first inline of each paragraph of the page at `file`.
fn inlines(site: &model::Site, file: &str) -> Vec<Inline> {
    let page = site.pages().iter().find(|p| p.file.as_str() == file).unwrap();
    page.body
        .blocks
        .iter()
        .filter_map(|b| if let Block::Paragraph(i) = b { i.first().cloned() } else { None })
        .collect()
}

fn target(inline: &Inline) -> &LinkTarget {
    match inline {
        Inline::Link { target, .. } => target,
        other => panic!("not a link: {other:?}"),
    }
}

#[test]
fn resolves_relative_paths_anchors_and_names() {
    let u = Tree::new()
        .file("posts/a.md", "[b](b.md#Some Heading)\n[up](../about.md)\n[bare](b)\n[root](/about.md)\n[here](#x)\n[out](https://example.org)\n")
        .file("posts/b.md", "x\n")
        .file("about.md", "x\n")
        .understand(&Pinned::none());
    assert!(codes(&u).is_empty(), "{:?}", u.diagnostics);
    let site = u.site.unwrap();
    let links = inlines(&site, "posts/a.md");
    let b = PageId::of(&path("posts/b.md"));
    let about = PageId::of(&path("about.md"));
    assert!(
        matches!(target(&links[0]), LinkTarget::Page { id, anchor: Some(a) } if *id == b && a.as_str() == "some-heading")
    );
    assert!(matches!(target(&links[1]), LinkTarget::Page { id, anchor: None } if *id == about));
    assert!(matches!(target(&links[2]), LinkTarget::Page { id, .. } if *id == b));
    assert!(matches!(target(&links[3]), LinkTarget::Page { id, .. } if *id == about));
    assert!(matches!(target(&links[4]), LinkTarget::Anchor(a) if a.as_str() == "x"));
    assert!(matches!(target(&links[5]), LinkTarget::External(_)));
}

#[test]
fn an_ambiguous_name_lists_its_candidates_and_stops_the_build() {
    let u = Tree::new()
        .file("a.md", "[x](Note)\n")
        .file("one/Note.md", "x\n")
        .file("two/Note.md", "x\n")
        .understand(&Pinned::none());
    assert_eq!(codes(&u), ["link.target.ambiguous"]);
    let Problem::LinkTargetAmbiguous { candidates, .. } = &u.diagnostics[0].problem else { panic!() };
    assert_eq!(candidates, &[path("one/Note.md"), path("two/Note.md")]);
    assert!(u.site.is_none());
}

#[test]
fn a_missing_target_stays_plain_text() {
    let u = Tree::new().file("a.md", "[x](nowhere.md)\n[js](javascript:alert(1))\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["link.target.missing", "link.target.missing"]);
    let site = u.site.unwrap();
    let links = inlines(&site, "a.md");
    assert!(matches!(target(&links[0]), LinkTarget::Unresolved { written } if written == "nowhere.md"));
    assert!(matches!(target(&links[1]), LinkTarget::Unresolved { .. }));
}

#[test]
fn an_unpublished_target_stays_plain_text() {
    let u = Tree::new()
        .file("a.md", "[x](b.md)\n[y](.hidden/c.md)\n")
        .file("b.md", "---\ndraft: true\n---\n")
        .file(".hidden/c.md", "x\n")
        .understand(&Pinned::none());
    assert_eq!(codes(&u), ["link.target.unpublished", "link.target.unpublished"]);
    let links = inlines(u.site.as_ref().unwrap(), "a.md");
    assert!(matches!(target(&links[0]), LinkTarget::Unresolved { .. }));
}

#[test]
fn only_images_a_published_page_shows_are_published() {
    let u = Tree::new()
        .file("a.md", "![ferris](img/Ferris Photo.PNG)\n[file](paper.pdf)\n")
        .file("b.md", "---\ndraft: true\n---\n![secret](img/secret.png)\n")
        .file("img/Ferris Photo.PNG", "png")
        .file("img/secret.png", "png")
        .file("paper.pdf", "pdf")
        .understand(&Pinned::none());
    assert_eq!(codes(&u), ["link.target.asset"]);
    let site = u.site.as_ref().unwrap();
    let assets: Vec<(&str, &str)> = site.assets().iter().map(|a| (a.file.as_str(), a.route.as_str())).collect();
    assert_eq!(assets, [("img/Ferris Photo.PNG", "/img/ferris-photo.png")]);
    let image = &inlines(site, "a.md")[0];
    assert!(
        matches!(image, Inline::Image { source: ImageSource::Asset(p), .. } if p.as_str() == "img/Ferris Photo.PNG")
    );
    assert!(!on::<code::Publish>(&u, "img/secret.png").value);
    assert!(site.excluded().iter().any(|e| e.path.as_str() == "img/secret.png"));
}

/// A format that returns one document for every file.
struct Fixed(model::Document);

impl engine::Format for Fixed {
    fn handles(&self, path: &model::ContentPath) -> bool {
        path.extension() == Some("md")
    }

    fn read(&self, _: &model::ContentPath, _: &[u8]) -> model::Read {
        model::Read::new(self.0.clone(), model::Front::default(), None)
    }
}

#[test]
fn links_in_list_items_and_footnotes_are_resolved() {
    let link = |written: &str| {
        Block::Paragraph(vec![Inline::Link {
            target: LinkTarget::Unresolved { written: written.into() },
            children: vec![],
        }])
    };
    let list = Block::List { ordered: None, items: vec![model::Item::new(vec![link("gone.md")])], tight: true };
    let name = model::FootnoteName::from(model::Slug::parse("n").unwrap());
    let document =
        model::Document::new(vec![list]).with_footnotes(vec![model::Footnote::new(name, vec![link("lost.md")])]);
    let u = Tree::new().file("a.md", "").read_with(&Pinned::none(), Fixed(document));
    let written: Vec<&str> = u
        .diagnostics
        .iter()
        .filter_map(
            |d| if let Problem::LinkTargetMissing { written } = &d.problem { Some(written.as_str()) } else { None },
        )
        .collect();
    assert_eq!(written, ["gone.md", "lost.md"]);
}
