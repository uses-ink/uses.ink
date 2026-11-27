//! What a render costs with the page cache (decision 0048): a generated blog of 10, 500 and
//! 2,000 posts rendered cold (a cache folder that holds nothing), warm (nothing changed) and
//! after an edit of one post, through the quiet theme and a cache folder on disk. Interpreting is
//! not in it: it is not cached. `cargo bench -p uses-ink-cli --bench incremental`.

#![allow(clippy::unwrap_used, reason = "a bench stops at the first surprise")]

use std::path::PathBuf;

use divan::{Bencher, black_box};
use engine::image::{Kept, Prepared, Source};
use engine::{BaseUrl, Engine, Format, Host, NoPins, RenderSettings, Rendered};
use fixture::corpus::Blog;
use host_native::{Disk, Grammars, Memory};
use model::{Bytes, ContentPath, Diagnostic, Dimensions, Entry, Hash, Observed, Read, Site, Snapshot, Version};
use theme_quiet::Quiet;

fn main() {
    divan::main();
}

/// `.md` files, read by the markdown format.
struct Markdown;

impl Format for Markdown {
    fn handles(&self, path: &ContentPath) -> bool {
        format_markdown::handles(path)
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        format_markdown::read(path, bytes)
    }
}

/// Every image served as it is: the sites hold none.
struct Asis;

impl engine::Images for Asis {
    type Refusal = Diagnostic;

    #[expect(clippy::result_large_err, reason = "a refusal is the diagnostic the engine records")]
    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Diagnostic>> {
        sources.iter().map(|_| Ok(Prepared::Kept { dimensions: Dimensions::new(1, 1), reason: Kept::Format })).collect()
    }
}

/// A generated blog, interpreted, with the store that holds its files.
struct Posts {
    files: Vec<(String, String)>,
}

impl Posts {
    fn new(count: usize) -> Self {
        Self { files: Blog { posts: count }.files() }
    }

    /// The site of the files, with `edited` appended to the post `at`.
    fn site(&self, at: usize, edited: u64) -> (Site, Memory) {
        let mut store = Memory::default();
        let tree = self
            .files
            .iter()
            .map(|(path, text)| {
                let text = if path == &format!("posts/post-{at}.md") {
                    format!("{text}\n\nEdit {edited}.\n")
                } else {
                    text.clone()
                };
                let size = Bytes(text.len() as u64);
                Entry::new(path.parse().unwrap(), store.insert(text.into_bytes()), size)
            })
            .collect();
        let version = Version::Folder { hash: Hash::of(b"bench") };
        let snapshot = Snapshot::new(version, tree, vec![], Observed::default()).unwrap();
        (Engine::new(&store, NoPins, (Markdown,), ()).interpret(&snapshot).site.unwrap(), store)
    }
}

fn folder(name: &str, count: usize) -> Disk {
    let path: PathBuf = [env!("CARGO_TARGET_TMPDIR"), "incremental", &format!("{name}-{count}")].iter().collect();
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    Disk::open(path)
}

fn render(site: &Site, store: &Memory, cache: &Disk) -> Rendered {
    let base: BaseUrl = "https://example.com".parse().unwrap();
    let theme = Quiet::new(base.clone(), Grammars::default());
    let engine = Engine::new(store, NoPins, (), &theme);
    engine.render_with(site, &RenderSettings { base }, Host::new(&Asis).cached(cache))
}

/// Every page rendered, the cache folder empty.
#[divan::bench(args = [10, 500, 2000], sample_count = 5)]
fn cold(bencher: Bencher<'_, '_>, count: usize) {
    let (site, store) = Posts::new(count).site(0, 0);
    let mut round = 0;
    bencher
        .with_inputs(|| {
            round += 1;
            folder(&format!("cold-{round}"), count)
        })
        .bench_local_refs(|cache| render(black_box(&site), &store, cache));
}

/// Nothing changed since the last build: every page taken.
#[divan::bench(args = [10, 500, 2000], sample_count = 5)]
fn warm(bencher: Bencher<'_, '_>, count: usize) {
    let (site, store) = Posts::new(count).site(0, 0);
    let cache = folder("warm", count);
    let first = render(&site, &store, &cache);
    assert!(first.steps.iter().all(|step| step.outcome == engine::Outcome::Rendered));
    bencher.bench(|| render(black_box(&site), &store, &cache));
}

/// One post edited since the last build: it and what reads it are rendered again.
#[divan::bench(args = [10, 500, 2000], sample_count = 5)]
fn edit(bencher: Bencher<'_, '_>, count: usize) {
    let posts = Posts::new(count);
    let (site, store) = posts.site(count / 2, 0);
    let cache = folder("edit", count);
    render(&site, &store, &cache);
    let mut edited = 0;
    bencher
        .with_inputs(|| {
            edited += 1;
            posts.site(count / 2, edited)
        })
        .bench_local_refs(|(site, store)| render(black_box(site), store, &cache));
}
