//! The render unit index: a page whose key the cache holds is not rendered, a page whose
//! inputs changed is, and what comes out is what a cold build gives, byte for byte.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Mutex;

use engine::cache::{Cache, Key};
use engine::{Engine, Host, Html, NoPins, Outcome, Reads, Record, Rendered, Theme};
use model::{Announcement, Diagnostic, File, Hash, Notice, Page, PageKind, Site};

use crate::support::{Plain, Stand, Store, Tree, settings};

/// A cache in memory that counts what is asked of it and can be made to lie.
#[derive(Debug, Default)]
struct Memory {
    kept: Mutex<BTreeMap<Key, Vec<Vec<u8>>>>,
    rejected: Mutex<Vec<Key>>,
    puts: Mutex<usize>,
}

impl Cache for Memory {
    fn get(&self, key: &Key) -> Option<Vec<Vec<u8>>> {
        self.kept.lock().unwrap().get(key).cloned()
    }

    fn put(&self, key: &Key, values: &[&[u8]]) {
        *self.puts.lock().unwrap() += 1;
        self.kept.lock().unwrap().insert(*key, values.iter().map(|value| value.to_vec()).collect());
    }

    fn reject(&self, key: &Key) {
        self.rejected.lock().unwrap().push(*key);
        self.kept.lock().unwrap().remove(key);
    }
}

/// The stand-in theme, saying who it is, what it reads, and which pages it was asked for.
#[derive(Debug)]
struct Counted {
    plain: Plain,
    identity: Option<Hash>,
    reads: Reads,
    /// Titles of pages it says something about, after which it writes them as usual.
    says: Vec<String>,
    asked: RefCell<Vec<String>>,
}

impl Counted {
    fn reading(reads: Reads) -> Self {
        Self {
            plain: Plain::default(),
            identity: Some(Hash::of(b"plain 1")),
            reads,
            says: vec![],
            asked: RefCell::default(),
        }
    }

    fn asked(&self) -> Vec<String> {
        let mut asked = self.asked.borrow().clone();
        asked.sort();
        asked
    }
}

impl Theme for Counted {
    fn name(&self) -> &'static str {
        "counted"
    }

    fn assets(&self) -> Vec<File> {
        self.plain.assets()
    }

    fn render<R: Record + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        self.asked.borrow_mut().push(page.title.clone());
        if self.says.contains(&page.title) {
            record.notice(Notice::new(Announcement::CodeLanguageUnknown { language: "said".to_owned() }));
        }
        self.plain.render(page, site, record)
    }

    fn identity(&self) -> Option<Hash> {
        self.identity
    }

    fn reads(&self, page: &Page) -> Reads {
        if self.reads == Reads::Linked && page.kind == PageKind::Home { Reads::Listed } else { self.reads }
    }
}

fn blog() -> Tree {
    Tree::new()
        .file("index.md", "---\ntitle: Ink\n---\nWelcome.\n")
        .file("about.md", "# About\n[the first](posts/first.md)\n")
        .file("posts/first.md", "---\ntitle: First\ndate: 2024-03-01\ntype: post\n---\nThe first post.\n")
        .file("posts/second.md", "---\ntitle: Second\ndate: 2024-04-02\ntype: post\n---\n[back](first.md)\n")
        .file("posts/third.md", "---\ntitle: Third\ndate: 2024-05-02\ntype: post\n---\nNothing links here.\n")
}

fn render(tree: &Tree, theme: &Counted, cache: &Memory) -> Rendered {
    let (_, store) = tree.snapshot();
    let site = tree.understand(&crate::support::Pinned::none()).site.unwrap();
    Engine::new(&store, NoPins, (), theme).render_with(&site, &settings(), Host::new(&Stand).cached(cache))
}

fn cold(tree: &Tree, theme: &Counted) -> Rendered {
    tree.render(theme)
}

fn titles(rendered: &Rendered, outcome: Outcome) -> Vec<String> {
    let mut found: Vec<String> =
        rendered.steps.iter().filter(|step| step.outcome == outcome).map(|step| step.unit.to_string()).collect();
    found.sort();
    found
}

fn same(a: &Rendered, b: &Rendered) {
    assert_eq!(a.manifest.id(), b.manifest.id());
    assert_eq!(a.blobs, b.blobs);
    assert_eq!(format!("{:?}", a.diagnostics), format!("{:?}", b.diagnostics));
}

#[test]
fn a_build_with_nothing_changed_renders_nothing() {
    let (theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    let first = render(&blog(), &theme, &cache);
    assert_eq!(titles(&first, Outcome::Reused), Vec::<String>::new());
    theme.asked.borrow_mut().clear();
    let second = render(&blog(), &theme, &cache);
    assert!(theme.asked().is_empty(), "asked for {:?}", theme.asked());
    assert_eq!(titles(&second, Outcome::Rendered), Vec::<String>::new());
    same(&first, &second);
    same(&second, &cold(&blog(), &theme));
}

#[test]
fn a_body_edit_renders_its_page_and_no_other() {
    let (theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    render(&blog(), &theme, &cache);
    theme.asked.borrow_mut().clear();
    let edited = blog()
        .file("posts/first.md", "---\ntitle: First\ndate: 2024-03-01\ntype: post\n---\nThe first post.\n\nMore.\n");
    let warm = render(&edited, &theme, &cache);
    assert_eq!(theme.asked(), ["First"]);
    assert_eq!(titles(&warm, Outcome::Rendered), ["posts/first.md"]);
    same(&warm, &cold(&edited, &theme));
}

#[test]
fn an_edit_of_the_summary_is_an_edit_of_what_neighbours_read() {
    let (theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    render(&blog(), &theme, &cache);
    let edited = blog().file("posts/first.md", "---\ntitle: First\ndate: 2024-03-01\ntype: post\n---\nChanged.\n");
    let warm = render(&edited, &theme, &cache);
    assert_eq!(titles(&warm, Outcome::Rendered), ["about.md", "index.md", "posts/first.md", "posts/second.md"]);
    same(&warm, &cold(&edited, &theme));
}

#[test]
fn a_title_edit_renders_the_page_its_linkers_and_the_listing() {
    let (theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    render(&blog(), &theme, &cache);
    theme.asked.borrow_mut().clear();
    let edited =
        blog().file("posts/first.md", "---\ntitle: Renamed\ndate: 2024-03-01\ntype: post\n---\nThe first post.\n");
    let warm = render(&edited, &theme, &cache);
    // about and second link to it; the home lists it; third and the not-found page do not read it
    assert_eq!(titles(&warm, Outcome::Rendered), ["about.md", "index.md", "posts/first.md", "posts/second.md"]);
    assert_eq!(titles(&warm, Outcome::Reused), ["404", "posts/third.md"]);
    same(&warm, &cold(&edited, &theme));
}

#[test]
fn a_theme_that_says_nothing_of_what_it_reads_renders_everything_when_anything_changes() {
    let (theme, cache) = (Counted::reading(Reads::Whole), Memory::default());
    render(&blog(), &theme, &cache);
    theme.asked.borrow_mut().clear();
    let edited = blog().file("posts/third.md", "---\ntitle: Third\ndate: 2024-05-02\ntype: post\n---\nChanged.\n");
    let warm = render(&edited, &theme, &cache);
    assert_eq!(titles(&warm, Outcome::Reused), Vec::<String>::new());
    same(&warm, &cold(&edited, &theme));
    theme.asked.borrow_mut().clear();
    render(&edited, &theme, &cache);
    assert_eq!(theme.asked(), Vec::<String>::new());
}

#[test]
fn a_page_the_theme_said_something_about_is_rendered_again_and_said_again() {
    let (mut theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    theme.says = vec!["Third".to_owned()];
    let first = render(&blog(), &theme, &cache);
    theme.asked.borrow_mut().clear();
    let second = render(&blog(), &theme, &cache);
    assert_eq!(theme.asked(), ["Third"]);
    let said = |rendered: &Rendered| {
        rendered
            .notices
            .iter()
            .filter(|notice| matches!(notice.announcement, Announcement::CodeLanguageUnknown { .. }))
            .count()
    };
    assert_eq!((said(&first), said(&second)), (1, 1));
}

#[test]
fn a_changed_theme_or_address_keys_every_page_again() {
    let (theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    render(&blog(), &theme, &cache);
    let mut other = Counted::reading(Reads::Linked);
    other.identity = Some(Hash::of(b"plain 2"));
    let warm = render(&blog(), &other, &cache);
    assert_eq!(titles(&warm, Outcome::Reused), Vec::<String>::new());
    let (_, store) = blog().snapshot();
    let site = blog().understand(&crate::support::Pinned::none()).site.unwrap();
    let mut elsewhere = settings();
    elsewhere.base = "https://elsewhere.example".parse().unwrap();
    let moved =
        Engine::new(&store, NoPins, (), &theme).render_with(&site, &elsewhere, Host::new(&Stand).cached(&cache));
    assert_eq!(titles(&moved, Outcome::Reused), Vec::<String>::new());
}

#[test]
fn a_theme_with_no_identity_keeps_nothing_and_says_so() {
    let (mut theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    theme.identity = None;
    let rendered = render(&blog(), &theme, &cache);
    assert_eq!(*cache.puts.lock().unwrap(), 0);
    assert!(rendered.notices.iter().any(|n| matches!(n.announcement, Announcement::RenderCacheUnkeyed { .. })));
}

#[test]
fn an_entry_that_is_not_one_value_is_dropped_and_the_page_rendered() {
    let (theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    let first = render(&blog(), &theme, &cache);
    for values in cache.kept.lock().unwrap().values_mut() {
        values.push(b"extra".to_vec());
    }
    theme.asked.borrow_mut().clear();
    let second = render(&blog(), &theme, &cache);
    assert_eq!(titles(&second, Outcome::Reused), Vec::<String>::new());
    assert_eq!(cache.rejected.lock().unwrap().len(), first.steps.len());
    same(&first, &second);
}

#[test]
fn the_report_counts_what_was_taken_and_what_was_rendered() {
    let (theme, cache) = (Counted::reading(Reads::Linked), Memory::default());
    let used = |rendered: &Rendered| {
        rendered.notices.iter().find_map(|notice| match &notice.announcement {
            Announcement::RenderCacheUsed { reused, rendered } => Some((*reused, *rendered)),
            _ => None,
        })
    };
    assert_eq!(used(&render(&blog(), &theme, &cache)), Some((0, 6)));
    assert_eq!(used(&render(&blog(), &theme, &cache)), Some((6, 0)));
    let none = Engine::new(&Store::default(), NoPins, (), &theme).render(
        &blog().understand(&crate::support::Pinned::none()).site.unwrap(),
        &settings(),
        &Stand,
    );
    assert_eq!(used(&none), None);
}
