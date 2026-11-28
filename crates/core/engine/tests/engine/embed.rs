//! Embeds and the names Obsidian links by: aliases, names in any case, partial paths, the
//! vault's attachment folder, and each refusal an embed can meet.

use model::{Announcement, Block, Inline, LinkTarget, PageId, Problem};

use crate::support::{Pinned, Tree, codes, path};

/// The blocks of the page at `file`.
fn blocks(understood: &engine::Understood, file: &str) -> Vec<Block> {
    let site = understood.site.as_ref().unwrap();
    site.pages().iter().find(|page| page.file.as_str() == file).unwrap().body.blocks.clone()
}

/// Where the link alone in a paragraph points.
fn linked(block: &Block) -> &LinkTarget {
    match block {
        Block::Paragraph(inlines) => match inlines.as_slice() {
            [Inline::Link { target, .. }] => target,
            other => panic!("not one link: {other:?}"),
        },
        other => panic!("not a paragraph: {other:?}"),
    }
}

#[test]
fn a_name_resolves_by_alias_in_any_case_and_by_a_partial_path() {
    let u = Tree::new()
        .file("a.md", "[x](Rusty)\n[y](rust)\n[z](notes/Rust)\n")
        .file("deep/notes/Rust.md", "---\naliases: Rusty, The language\n---\n")
        .understand(&Pinned::none());
    assert!(codes(&u).is_empty(), "{:?}", u.diagnostics);
    let rust = PageId::of(&path("deep/notes/Rust.md"));
    for block in blocks(&u, "a.md") {
        assert!(matches!(linked(&block), LinkTarget::Page { id, .. } if *id == rust), "{block:?}");
    }
}

#[test]
fn an_embed_shows_the_section_or_block_it_names() {
    let u = Tree::new()
        .file("a.md", "![[b#Part]]\n![[b#^kept]]\n")
        .file("b.md", "before\n# Part\ninside\n^kept\n# Next\nafter\n")
        .understand(&Pinned::none());
    assert!(codes(&u).is_empty(), "{:?}", u.diagnostics);
    let shown = blocks(&u, "a.md");
    let Block::Embed { blocks: section, .. } = &shown[0] else { panic!("{:?}", shown[0]) };
    // the heading and what follows it up to the next, the block id removed
    assert_eq!(section.len(), 3, "{section:?}");
    assert!(matches!(&section[0], Block::Heading { anchor, .. } if anchor.as_str() == "part"));
    assert_eq!(section[2], Block::Paragraph(Vec::new()));
    let Block::Embed { blocks: kept, .. } = &shown[1] else { panic!("{:?}", shown[1]) };
    assert_eq!(kept, &[Block::Paragraph(vec![Inline::Text("inside".to_owned())])]);
}

#[test]
fn an_embed_of_itself_is_a_cycle_shown_as_a_link() {
    let u = Tree::new().file("a.md", "![[b]]\n").file("b.md", "![[a]]\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["embed.target.cycle", "embed.target.cycle"]);
    let Problem::EmbedTargetCycle { files, .. } = &u.diagnostics[0].problem else { panic!() };
    assert_eq!(files, &[path("a.md"), path("b.md")]);
    let shown = blocks(&u, "a.md");
    let Block::Embed { blocks: inner, .. } = &shown[0] else { panic!() };
    assert!(matches!(linked(&inner[0]), LinkTarget::Page { id, .. } if *id == PageId::of(&path("a.md"))));
}

#[test]
fn an_embed_naming_no_part_is_a_link() {
    let u = Tree::new().file("a.md", "![[b#Nowhere]]\n![[c]]\n").file("b.md", "x\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["link.target.missing", "embed.anchor.missing"]);
    let shown = blocks(&u, "a.md");
    assert!(matches!(linked(&shown[0]), LinkTarget::Page { anchor: Some(anchor), .. } if anchor.as_str() == "nowhere"));
    assert!(matches!(linked(&shown[1]), LinkTarget::Unresolved { written } if written == "c"));
}

#[test]
fn embeds_past_the_budget_are_links() {
    let many = "![[b]]\n".repeat(65);
    let u = Tree::new().file("a.md", &many).file("b.md", "x\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["embed.budget.count"]);
    let shown = blocks(&u, "a.md");
    assert!(matches!(&shown[63], Block::Embed { .. }));
    assert!(matches!(linked(&shown[64]), LinkTarget::Page { .. }));
}

#[test]
fn the_attachment_folder_settles_a_shared_name_and_says_so() {
    let u = Tree::new()
        .file(".obsidian/app.json", r#"{"attachmentFolderPath": "files"}"#)
        .file("a.md", "---\npublish: true\n---\n![x](leaf.png)\n")
        .file("files/leaf.png", "png")
        .file("old/leaf.png", "png")
        .understand(&Pinned::none());
    assert!(codes(&u).is_empty(), "{:?}", u.diagnostics);
    let attached = u.notices.iter().find_map(|notice| match &notice.announcement {
        Announcement::LinkTargetAttached { target, candidates, .. } => Some((target.clone(), candidates.clone())),
        _ => None,
    });
    assert_eq!(attached, Some((path("files/leaf.png"), vec![path("files/leaf.png"), path("old/leaf.png")])));
}

#[test]
fn settings_that_do_not_read_are_said_and_leave_names_ambiguous() {
    let u = Tree::new()
        .file(".obsidian/app.json", "{ not json")
        .file("a.md", "---\npublish: true\n---\n![x](leaf.png)\n")
        .file("files/leaf.png", "png")
        .file("old/leaf.png", "png")
        .understand(&Pinned::none());
    assert_eq!(codes(&u), ["vault.settings.invalid", "link.target.ambiguous"]);
}

/// The text of an inline, for naming what a paragraph holds.
fn said(inline: &Inline) -> String {
    match inline {
        Inline::Text(text) => text.clone(),
        Inline::Link { children, .. } => children.iter().map(said).collect(),
        other => panic!("not text: {other:?}"),
    }
}

#[test]
fn an_embed_among_text_ends_the_paragraph_and_shows_the_page() {
    let u = Tree::new()
        .file("a.md", "~ see ![[b]] here, and ![[c]] there\n")
        .file("b.md", "bee\n")
        .file("c.md", "sea\n")
        .understand(&Pinned::none());
    assert!(codes(&u).is_empty(), "{:?}", u.diagnostics);
    let shown = blocks(&u, "a.md");
    assert_eq!(shown.len(), 5, "{shown:?}");
    assert!(matches!(&shown[0], Block::Paragraph(text) if said(&text[0]) == "see "));
    assert!(matches!(&shown[1], Block::Embed { target: LinkTarget::Page { id, .. }, blocks }
            if *id == PageId::of(&path("b.md")) && blocks == &[Block::Paragraph(vec![Inline::Text("bee".to_owned())])]));
    assert!(matches!(&shown[2], Block::Paragraph(text) if said(&text[0]) == " here, and "));
    assert!(matches!(&shown[3], Block::Embed { blocks, .. } if !blocks.is_empty()));
    assert!(matches!(&shown[4], Block::Paragraph(text) if said(&text[0]) == " there"));
}

#[test]
fn an_embed_among_text_keeps_the_cycle_and_budget_refusals() {
    let u = Tree::new().file("a.md", "~ x ![[b]] y\n").file("b.md", "~ ![[a]]\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["embed.target.cycle", "embed.target.cycle"]);
    let many = "~ x ![[b]] y\n".repeat(65);
    let u = Tree::new().file("a.md", &many).file("b.md", "z\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["embed.budget.count"]);
    let embeds = blocks(&u, "a.md")
        .iter()
        .filter(|block| matches!(block, Block::Embed { blocks, .. } if !blocks.is_empty()))
        .count();
    assert_eq!(embeds, 64);
}

#[test]
fn an_embed_among_text_naming_nothing_stays_a_link_in_the_text() {
    let u = Tree::new().file("a.md", "~ x ![[nowhere]] y\n").understand(&Pinned::none());
    assert_eq!(codes(&u), ["link.target.missing"]);
    let shown = blocks(&u, "a.md");
    let Block::Paragraph(inlines) = &shown[0] else { panic!("{shown:?}") };
    assert!(
        matches!(&inlines[1], Inline::Link { target: LinkTarget::Unresolved { written }, .. } if written == "nowhere")
    );
    assert_eq!(shown.len(), 1);
}

#[test]
fn an_embed_where_only_text_fits_is_a_link_and_says_so() {
    let u = Tree::new().file("a.md", "#~ x ![[b]]\n").file("b.md", "bee\n").understand(&Pinned::none());
    assert_eq!(codes(&u), Vec::<&str>::new());
    assert!(
        u.notices.iter().any(
            |notice| matches!(&notice.announcement, Announcement::EmbedInlineLinked { written } if written == "b")
        )
    );
    let shown = blocks(&u, "a.md");
    let Block::Heading { inlines, .. } = &shown[0] else { panic!("{shown:?}") };
    assert!(matches!(&inlines[1], Inline::Link { target: LinkTarget::Page { .. }, .. }));
}

#[test]
fn an_embedded_heading_gets_an_id_the_page_does_not_have() {
    let u = Tree::new()
        .file("a.md", "# Part\n![[b]]\n![[b]]\n![[b#Other]]\n")
        .file("b.md", "# Part\ntext\n# Other\n")
        .understand(&Pinned::none());
    assert!(codes(&u).is_empty(), "{:?}", u.diagnostics);
    let mut ids = Vec::new();
    let mut anchors = |blocks: &[Block]| {
        for block in blocks {
            match block {
                Block::Heading { anchor, .. } => ids.push(anchor.as_str().to_owned()),
                Block::Embed { blocks, .. } => {
                    for block in blocks {
                        if let Block::Heading { anchor, .. } = block {
                            ids.push(anchor.as_str().to_owned());
                        }
                    }
                }
                _ => {}
            }
        }
    };
    anchors(&blocks(&u, "a.md"));
    // the page's own `part` comes first; each embed takes what is left, in document order
    assert_eq!(ids, ["part", "part-1", "other", "part-2", "other-1", "other-2"]);
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn an_embedded_link_to_its_own_heading_still_points_at_its_page() {
    let u =
        Tree::new().file("a.md", "# Part\n![[b]]\n").file("b.md", "# Part\n[go](#part)\n").understand(&Pinned::none());
    assert!(codes(&u).is_empty(), "{:?}", u.diagnostics);
    let shown = blocks(&u, "a.md");
    let Block::Embed { blocks: inner, .. } = &shown[1] else { panic!("{shown:?}") };
    let b = PageId::of(&path("b.md"));
    assert!(
        matches!(linked(&inner[1]), LinkTarget::Page { id, anchor: Some(anchor) } if *id == b && anchor.as_str() == "part")
    );
}
