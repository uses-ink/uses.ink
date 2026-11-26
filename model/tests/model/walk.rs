use model::{
    Alignment, Anchor, Block, CalloutKind, ContentPath, Definition, Document, Fold, Footnote, FootnoteName,
    ImageSource, Inline, Item, Level, LinkTarget, PageId, Slug, Task, Visit,
};

fn text(value: &str) -> Inline {
    Inline::Text(value.to_owned())
}

fn unresolved(written: &str, label: &str) -> Inline {
    Inline::Link { target: LinkTarget::Unresolved { written: written.to_owned() }, children: vec![text(label)] }
}

fn slug(text: &str) -> Slug {
    Slug::parse(text).unwrap()
}

/// One of every inline kind, a link among them.
fn every_inline() -> Vec<Inline> {
    vec![
        text("A"),
        Inline::Emphasis(vec![text("B")]),
        Inline::Strong(vec![Inline::Strike(vec![text("C")])]),
        Inline::Highlight(vec![unresolved("one.md", "D")]),
        Inline::Superscript(vec![text("E")]),
        Inline::Subscript(vec![text("F")]),
        Inline::Code("G".to_owned()),
        Inline::Keyboard("H".to_owned()),
        Inline::Math("x".to_owned()),
        Inline::Break,
        Inline::SoftBreak,
        Inline::Footnote(FootnoteName::from(slug("n"))),
        Inline::Image { source: ImageSource::Unresolved { written: "i.png".to_owned() }, alt: "alt".to_owned() },
    ]
}

/// One of every node kind, with a link in each place a link can be.
fn every_node() -> Document {
    let heading =
        Block::Heading { level: Level::new(1).unwrap(), anchor: Anchor::new(slug("top")), inlines: vec![text("Top")] };
    let paragraph = Block::Paragraph(every_inline());
    let list = Block::List {
        ordered: None,
        items: vec![
            Item::new(vec![Block::Paragraph(vec![text("I")])]),
            Item::task(Task::Done, vec![Block::Paragraph(vec![unresolved("two.md", "J")])]),
        ],
        tight: true,
    };
    let table = Block::Table {
        alignments: vec![Alignment::Left],
        head: vec![vec![text("K")]],
        rows: vec![vec![vec![unresolved("three.md", "L")]]],
    };
    let callout = Block::Callout {
        kind: CalloutKind::Note,
        title: Some(vec![text("M")]),
        blocks: vec![Block::Quote(vec![Block::Paragraph(vec![text("N")])])],
    };
    let definitions = Block::Definitions(vec![Definition::new(
        vec![text("O")],
        vec![vec![Block::Paragraph(vec![unresolved("four.md", "P")])]],
    )]);
    let details = Block::Details {
        summary: vec![text("Q")],
        blocks: vec![Block::Code { language: None, text: "code".to_owned() }, Block::Rule, Block::Math("y".to_owned())],
    };
    let footnote =
        Footnote::new(FootnoteName::from(slug("n")), vec![Block::Paragraph(vec![unresolved("five.md", "R")])]);
    Document::new(vec![heading, paragraph, list, table, callout, definitions, details]).with_footnotes(vec![footnote])
}

/// Every plain text and inline code, in reading order.
#[derive(Default)]
struct Plain(String);

impl Visit for Plain {
    fn inline(&mut self, inline: &Inline) {
        if let Inline::Code(code) | Inline::Keyboard(code) = inline {
            self.0.push_str(code);
        }
        inline.walk(self);
    }

    fn text(&mut self, text: &str) {
        self.0.push_str(text);
    }
}

/// Links, by whether they resolved.
#[derive(Default)]
struct Links {
    unresolved: usize,
    pages: usize,
}

impl Visit for Links {
    fn link(&mut self, target: &LinkTarget, children: &[Inline]) {
        match target {
            LinkTarget::Unresolved { .. } => self.unresolved += 1,
            LinkTarget::Page { .. } => self.pages += 1,
            _ => {}
        }
        self.inlines(children);
    }
}

/// Resolves every unresolved link to the page of the file it names.
struct Resolve;

impl Fold for Resolve {
    fn link(&mut self, target: LinkTarget, children: Vec<Inline>) -> Inline {
        let target = match target {
            LinkTarget::Unresolved { written } => {
                LinkTarget::Page { id: PageId::of(&ContentPath::parse(written).unwrap()), anchor: None }
            }
            other => other,
        };
        Inline::Link { target, children: self.inlines(children) }
    }
}

#[test]
fn a_visit_reaches_every_text_in_reading_order() {
    let mut plain = Plain::default();
    plain.document(&every_node());
    assert_eq!(plain.0, "TopABCDEFGHIJKLMNOPQR");
}

#[test]
fn a_visit_counts_links_in_every_place() {
    let mut links = Links::default();
    links.document(&every_node());
    assert_eq!((links.unresolved, links.pages), (5, 0));
}

#[test]
fn a_fold_resolves_links_and_keeps_everything_else() {
    let resolved = Resolve.document(every_node());
    let mut links = Links::default();
    links.document(&resolved);
    assert_eq!((links.unresolved, links.pages), (0, 5));
    let mut before = Plain::default();
    before.document(&every_node());
    let mut after = Plain::default();
    after.document(&resolved);
    assert_eq!(before.0, after.0);
    assert_eq!(resolved.footnotes.len(), 1);
    assert_ne!(resolved, every_node());
}

#[test]
fn the_default_fold_changes_nothing() {
    struct Same;
    impl Fold for Same {}
    assert_eq!(Same.document(every_node()), every_node());
}
