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
