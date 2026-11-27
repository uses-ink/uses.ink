//! Incremental builds on the site fixtures: after any single edit of one file, a build that takes
//! what it can from the cache gives the deploy a cold build gives, and an untouched page is not
//! rendered again (decision 0045). The pages are rendered by `ink:theme-quiet`, so what its
//! `reads` say is held to what it reads.
//!
//! The properties run the theme over several fixtures and many edits: the file's name has
//! `incremental` in it for the budget in `.config/nextest.toml`.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use engine::cache::{Cache, Key};
use engine::image::{Kept, Prepared, Source};
use engine::{Announcement, Engine, Format, Host, NoPins, Outcome, RenderSettings, Rendered};
use fixture::corpus::Blog;
use host_native::{Grammars, Memory};
use model::{Bytes, ContentPath, Diagnostic, Dimensions, Entry, Hash, Notice, Observed, Read, Snapshot, Version};
use proptest::prelude::*;
use theme_quiet::Quiet;

/// `.md` files, read by the markdown format, as the CLI reads them.
struct Markdown;

impl Format for Markdown {
    fn handles(&self, path: &ContentPath) -> bool {
        format_markdown::handles(path)
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        format_markdown::read(path, bytes)
    }
}

/// Every image served as it is: the host's image work is measured on its own.
struct Asis;

impl engine::Images for Asis {
    type Refusal = Diagnostic;

    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Diagnostic>> {
        let dimensions = Dimensions::new(40, 30);
        sources.iter().map(|_| Ok(Prepared::Kept { dimensions, reason: Kept::Format })).collect()
    }
}

/// A cache in memory.
#[derive(Debug, Default)]
struct Memorized(Mutex<BTreeMap<Key, Vec<Vec<u8>>>>);

impl Clone for Memorized {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.0.lock().unwrap().clone()))
    }
}

impl Cache for Memorized {
    fn get(&self, key: &Key) -> Option<Vec<Vec<u8>>> {
        self.0.lock().unwrap().get(key).cloned()
    }

    fn put(&self, key: &Key, values: &[&[u8]]) {
        self.0.lock().unwrap().insert(*key, values.iter().map(|value| value.to_vec()).collect());
    }

    fn reject(&self, key: &Key) {
        self.0.lock().unwrap().remove(key);
    }
}

/// A folder of files, held in memory so one is edited and built without touching a disk.
#[derive(Clone, Debug)]
struct Tree(BTreeMap<String, Vec<u8>>);

/// What building a tree gave.
struct Built {
    rendered: Rendered,
}

impl Tree {
    fn of(dir: &Path) -> Self {
        let mut files = BTreeMap::new();
        Self::walk(dir, dir, &mut files);
        Self(files)
    }

    fn walk(dir: &Path, base: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                Self::walk(&path, base, files);
            } else {
                let name = path.strip_prefix(base).unwrap().to_str().unwrap().to_owned();
                files.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }

    fn generated(posts: usize) -> Self {
        Self(Blog { posts }.files().into_iter().map(|(path, text)| (path, text.into_bytes())).collect())
    }

    fn markdown(&self) -> Vec<&String> {
        self.0.keys().filter(|name| name.ends_with(".md")).collect()
    }

    /// Builds it through the quiet theme, taking and keeping pages in `cache` when there is one.
    /// `None` when the site did not interpret: an edit may break a fixture on purpose or not.
    fn build(&self, cache: Option<&Memorized>) -> Option<Built> {
        let mut store = Memory::default();
        let tree = self
            .0
            .iter()
            .map(|(path, bytes)| {
                let size = Bytes(bytes.len() as u64);
                Entry::new(path.parse().unwrap(), store.insert(bytes.clone()), size)
            })
            .collect();
        let version = Version::Folder { hash: Hash::of(b"incremental") };
        let snapshot = Snapshot::new(version, tree, vec![], Observed::default()).unwrap();
        let site = Engine::new(&store, NoPins, (Markdown,), ()).interpret(&snapshot).site?;
        let base: engine::BaseUrl = "https://ink.example".parse().unwrap();
        let theme = Quiet::new(base.clone(), Grammars::default());
        let engine = Engine::new(&store, NoPins, (), &theme);
        let settings = RenderSettings { base };
        let rendered = match cache {
            Some(cache) => engine.render_with(&site, &settings, Host::new(&Asis).cached(cache)),
            None => engine.render(&site, &settings, &Asis),
        };
        Some(Built { rendered })
    }
}

impl Built {
    /// The files of the render that were rendered again, as their paths.
    fn rendered(&self) -> Vec<String> {
        let mut files: Vec<String> = self
            .rendered
            .steps
            .iter()
            .filter(|step| step.outcome == Outcome::Rendered)
            .map(|step| step.unit.to_string())
            .collect();
        files.sort();
        files
    }

    /// The notices that are about the cache and nothing else.
    fn cached(notice: &Notice) -> bool {
        matches!(notice.announcement, Announcement::RenderCacheUsed { .. })
    }

    /// Asserts this build and `cold` are the same deploy, with the same bytes and the same
    /// things said, but for the cache's own notice.
    fn assert_same(&self, cold: &Self) {
        assert_eq!(self.rendered.manifest.id(), cold.rendered.manifest.id());
        assert_eq!(self.rendered.blobs, cold.rendered.blobs);
        assert_eq!(format!("{:?}", self.rendered.diagnostics), format!("{:?}", cold.rendered.diagnostics));
        let said = |built: &Self| {
            built
                .rendered
                .notices
                .iter()
                .filter(|notice| !Self::cached(notice))
                .map(|n| format!("{n:?}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(said(self), said(cold));
    }
}

/// One edit of one file.
#[derive(Clone, Debug)]
enum Edit {
    /// A paragraph at the end of the file.
    Append,
    /// A new first heading, which is the title when the file states none.
    Retitle,
    /// A link to another file.
    Link,
    /// The file is gone.
    Remove,
    /// A new file beside it, linking to it.
    Add,
    /// The bytes of another file change: the images of the blog are among them.
    Bytes,
}

impl Edit {
    /// The path that reaches `to` from the file `from`.
    fn reach(from: &str, to: &str) -> String {
        let (from, to): (Vec<&str>, Vec<&str>) = (from.split('/').collect(), to.split('/').collect());
        let (from_dir, to_dir) = (&from[..from.len() - 1], &to[..to.len() - 1]);
        let shared = from_dir.iter().zip(to_dir).take_while(|(a, b)| a == b).count();
        let up = vec![".."; from_dir.len() - shared].join("/");
        let down = to[shared..].join("/");
        if up.is_empty() { down } else { format!("{up}/{down}") }
    }

    /// After its front matter, where a heading is read.
    fn after_front(text: &str) -> usize {
        text.strip_prefix("---\n").and_then(|rest| rest.find("\n---\n").map(|end| 4 + end + 5)).unwrap_or(0)
    }

    fn apply(&self, tree: &Tree, file: &str, other: &str, n: u32) -> Tree {
        let mut edited = tree.clone();
        let text = String::from_utf8(edited.0[file].clone()).unwrap();
        match self {
            Self::Append => {
                edited.0.insert(file.to_owned(), format!("{text}\n\nAdded paragraph {n}.\n").into_bytes());
            }
            Self::Retitle => {
                let at = Self::after_front(&text);
                let retitled = format!("{}\n# Renamed {n}\n\n{}", &text[..at], &text[at..]);
                edited.0.insert(file.to_owned(), retitled.into_bytes());
            }
            Self::Link => {
                let link = format!("{text}\n\n[see](<{}>)\n", Self::reach(file, other));
                edited.0.insert(file.to_owned(), link.into_bytes());
            }
            Self::Remove => {
                edited.0.remove(file);
            }
            Self::Bytes => {
                let target = tree.0.keys().find(|name| !name.ends_with(".md") && name.as_str() >= other);
                if let Some(target) = target.or_else(|| tree.0.keys().find(|name| !name.ends_with(".md"))) {
                    let mut bytes = edited.0[target].clone();
                    bytes.extend(n.to_le_bytes());
                    edited.0.insert(target.clone(), bytes);
                }
            }
            Self::Add => {
                let dir = file.rsplit_once('/').map_or(String::new(), |(dir, _)| format!("{dir}/"));
                let added = format!("{dir}added-{n}.md");
                let body = format!("# Added {n}\n\n[it](<{}>)\n", Self::reach(&added, file));
                edited.0.insert(added, body.into_bytes());
            }
        }
        edited
    }
}

fn edit() -> impl Strategy<Value = (Edit, usize, usize, u32)> {
    let kind =
        prop_oneof![Just(Edit::Append), Just(Edit::Retitle), Just(Edit::Link), Just(Edit::Remove), Just(Edit::Add),];
    (kind, 0..64usize, 0..64usize, 0..1000u32)
}

fn fixtures() -> &'static [(&'static str, Tree)] {
    static FIXTURES: OnceLock<Vec<(&'static str, Tree)>> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        ["blog", "docs", "notes", "readme"]
            .into_iter()
            .map(|name| (name, Tree::of(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site").join(name))))
            .collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    /// Any single edit of a fixture, built on top of the cache the unedited fixture left, gives
    /// what a cold build of the edited fixture gives.
    #[test]
    fn an_incremental_build_after_one_edit_is_the_cold_build(
        (kind, at, with, n) in edit(),
        fixture in 0..4usize,
    ) {
        let (name, tree) = &fixtures()[fixture];
        let files = tree.markdown();
        let (file, other) = (files[at % files.len()].clone(), files[with % files.len()].clone());
        let cache = Memorized::default();
        tree.build(Some(&cache)).unwrap();
        let edited = kind.apply(tree, &file, &other, n);
        let cold = edited.build(None);
        let warm = edited.build(Some(&cache));
        prop_assert_eq!(cold.is_some(), warm.is_some(), "{} {:?} {}", name, kind, file);
        if let (Some(cold), Some(warm)) = (cold, warm) {
            warm.assert_same(&cold);
        }
    }
}

#[test]
fn a_build_of_the_same_fixture_renders_no_page() {
    for (name, tree) in fixtures() {
        let cache = Memorized::default();
        let first = tree.build(Some(&cache)).unwrap();
        assert!(!first.rendered().is_empty(), "{name}");
        let second = tree.build(Some(&cache)).unwrap();
        assert_eq!(second.rendered(), Vec::<String>::new(), "{name}");
        second.assert_same(&tree.build(None).unwrap());
    }
}

#[test]
fn an_edit_of_one_post_renders_that_post_and_no_other() {
    let tree = Tree::generated(40);
    let cache = Memorized::default();
    tree.build(Some(&cache)).unwrap();
    let edited = Edit::Append.apply(&tree, "posts/post-7.md", "", 1);
    let warm = edited.build(Some(&cache)).unwrap();
    assert_eq!(warm.rendered(), ["posts/post-7.md"]);
    warm.assert_same(&edited.build(None).unwrap());
}

#[test]
fn a_new_title_renders_the_page_the_pages_that_link_to_it_and_the_index() {
    let tree = Tree::generated(40);
    let cache = Memorized::default();
    tree.build(Some(&cache)).unwrap();
    let edited = Edit::Retitle.apply(&tree, "posts/post-7.md", "", 1);
    let warm = edited.build(Some(&cache)).unwrap();
    let rendered = warm.rendered();
    assert!(rendered.contains(&"posts/post-7.md".to_owned()));
    assert!(rendered.contains(&"index".to_owned()), "the generated home lists it: {rendered:?}");
    assert!(rendered.len() < 30, "{} of 42 pages", rendered.len());
    warm.assert_same(&edited.build(None).unwrap());
}

/// Writes the generated sites the performance notes measure with `ink build`, under
/// `target/tmp/synthetic/<posts>`: `cargo test -p uses-ink-cli --release --test incremental --
/// --ignored`.
#[test]
#[ignore = "writes generated sites for measuring `ink build`; not a check"]
fn writes_the_synthetic_sites() {
    for posts in [500, 2000] {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("synthetic").join(posts.to_string());
        for (path, bytes) in &Tree::generated(posts).0 {
            let target = root.join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
    }
}

/// A build of `tree` through a cache folder: what it rendered, and what the folder said.
fn on_disk(tree: &Tree, folder: &Path) -> (Built, Vec<Notice>) {
    let disk = host_native::Disk::open(folder);
    let mut store = Memory::default();
    let entries = tree
        .0
        .iter()
        .map(|(path, bytes)| Entry::new(path.parse().unwrap(), store.insert(bytes.clone()), Bytes(bytes.len() as u64)))
        .collect();
    let version = Version::Folder { hash: Hash::of(b"incremental") };
    let snapshot = Snapshot::new(version, entries, vec![], Observed::default()).unwrap();
    let site = Engine::new(&store, NoPins, (Markdown,), ()).interpret(&snapshot).site.unwrap();
    let base: engine::BaseUrl = "https://ink.example".parse().unwrap();
    let theme = Quiet::new(base.clone(), Grammars::default());
    let rendered = Engine::new(&store, NoPins, (), &theme).render_with(
        &site,
        &RenderSettings { base },
        Host::new(&Asis).cached(&disk),
    );
    (Built { rendered }, disk.notices())
}

#[test]
fn a_cache_folder_says_it_is_cold_once_and_a_damaged_page_is_rendered_again() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("incremental-disk");
    if folder.exists() {
        std::fs::remove_dir_all(&folder).unwrap();
    }
    let tree = Tree::generated(6);
    let (first, said) = on_disk(&tree, &folder);
    assert!(said.iter().any(|n| matches!(n.announcement, Announcement::RenderCacheCold { .. })), "{said:?}");
    let (second, said) = on_disk(&tree, &folder);
    assert!(second.rendered().is_empty());
    assert!(said.is_empty(), "{said:?}");
    first.assert_same(&second);

    // damage the bytes of one page, keeping the name its entry knows them by
    let page = std::fs::read_dir(folder.join("value")).unwrap().map(|entry| entry.unwrap().path()).next().unwrap();
    std::fs::write(&page, b"damaged").unwrap();
    let (third, said) = on_disk(&tree, &folder);
    assert!(said.iter().any(|n| matches!(n.announcement, Announcement::CacheEntryCorrupt { .. })), "{said:?}");
    assert_eq!(third.rendered().len(), 1);
    third.assert_same(&tree.build(None).unwrap());
}

#[test]
fn a_changed_image_renders_the_page_that_shows_it() {
    let tree = fixtures().iter().find(|(name, _)| *name == "blog").unwrap().1.clone();
    let cache = Memorized::default();
    tree.build(Some(&cache)).unwrap();
    let mut edited = tree.clone();
    edited.0.get_mut("posts/leaf.png").unwrap().push(7);
    let warm = edited.build(Some(&cache)).unwrap();
    assert_eq!(warm.rendered(), ["posts/pictures.md"]);
    warm.assert_same(&edited.build(None).unwrap());
}
