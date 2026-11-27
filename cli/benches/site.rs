//! How long a site takes, through the markdown format, the quiet theme and the host's grammars,
//! as `ink build` runs them: interpreting and rendering a blog of 500 posts, one post's page,
//! the 1.6 MB flood as a site of one page, a page of 20,000 footnotes, and highlighting code.
//! `cargo bench -p uses-ink-cli`.

#![allow(clippy::unwrap_used, reason = "a bench stops at the first surprise")]

use divan::{Bencher, black_box};
use engine::image::{Prepared, Source};
use engine::{BaseUrl, Engine, Format, NoPins, RenderSettings, Said, Theme};
use fixture::corpus::{Blog, CODE, Flood, Notes};
use host_native::{Grammars, Memory};
use model::highlight::Highlight;
use model::{Bytes, ContentPath, Diagnostic, Entry, Hash, Observed, Read, Site, Snapshot, Version};
use theme_quiet::Quiet;

fn main() {
    divan::main();
}

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

/// The host's images, for sites that have none.
struct Imageless;

impl engine::Images for Imageless {
    type Refusal = Diagnostic;

    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Diagnostic>> {
        assert!(sources.is_empty(), "the benchmark sites hold no images");
        Vec::new()
    }
}

/// A snapshot of `files` and the store holding their bytes.
fn snapshot(files: Vec<(String, String)>) -> (Snapshot, Memory) {
    let mut store = Memory::default();
    let tree = files
        .into_iter()
        .map(|(path, text)| {
            let size = Bytes(text.len() as u64);
            let hash = store.insert(text.into_bytes());
            Entry::new(path.parse().unwrap(), hash, size)
        })
        .collect();
    let version = Version::Folder { hash: Hash::of(b"bench") };
    (Snapshot::new(version, tree, vec![], Observed::default()).unwrap(), store)
}

fn base() -> BaseUrl {
    "https://example.com".parse().unwrap()
}

fn blog() -> (Snapshot, Memory) {
    snapshot(Blog { posts: 500 }.files())
}

fn site(snapshot: &Snapshot, store: &Memory) -> Site {
    Engine::new(store, NoPins, (Markdown,), ()).interpret(snapshot).site.unwrap()
}

/// Interpreting 500 posts: reading each, deciding everything, resolving links.
#[divan::bench(sample_count = 10)]
fn interpret(bencher: Bencher<'_, '_>) {
    let (snapshot, store) = blog();
    let engine = Engine::new(&store, NoPins, (Markdown,), ());
    bencher.bench(|| engine.interpret(black_box(&snapshot)));
}

/// Rendering 500 posts already interpreted: every page, the feeds, the sitemap.
#[divan::bench(sample_count = 10)]
fn render(bencher: Bencher<'_, '_>) {
    let (snapshot, store) = blog();
    let site = site(&snapshot, &store);
    let theme = Quiet::new(base(), Grammars::default());
    let engine = Engine::new(&store, NoPins, (), &theme);
    let settings = RenderSettings { base: base() };
    bencher.bench(|| engine.render(black_box(&site), &settings, &Imageless));
}

/// One post's page of the 500, through the theme alone.
#[divan::bench]
fn page(bencher: Bencher<'_, '_>) {
    let (snapshot, store) = blog();
    let site = site(&snapshot, &store);
    let theme = Quiet::new(base(), Grammars::default());
    let page = site.pages().iter().find(|page| page.file.as_str() == "posts/post-1.md").unwrap();
    bencher.bench(|| theme.render(black_box(page), &site, &mut Said::default()).unwrap());
}

/// The flood of 20,000 paragraphs as a site of one page, interpreted and rendered.
#[divan::bench(sample_count = 10)]
fn flood(bencher: Bencher<'_, '_>) {
    let (snapshot, store) = snapshot(vec![("flood.md".to_owned(), Flood { paragraphs: 20_000 }.to_string())]);
    let theme = Quiet::new(base(), Grammars::default());
    let settings = RenderSettings { base: base() };
    bencher.bench(|| {
        let site = Engine::new(&store, NoPins, (Markdown,), ()).interpret(black_box(&snapshot)).site.unwrap();
        Engine::new(&store, NoPins, (), &theme).render(&site, &settings, &Imageless)
    });
}

/// A page of 20,000 footnotes, each referenced once, through the theme alone.
#[divan::bench(sample_count = 10)]
fn notes(bencher: Bencher<'_, '_>) {
    let (snapshot, store) = snapshot(vec![("notes.md".to_owned(), Notes { count: 20_000 }.to_string())]);
    let site = site(&snapshot, &store);
    let theme = Quiet::new(base(), Grammars::default());
    let page = site.pages().iter().find(|page| page.file.as_str() == "notes.md").unwrap();
    bencher.bench(|| theme.render(black_box(page), &site, &mut Said::default()).unwrap());
}

/// One block of code of each language a post quotes, its grammar already compiled.
#[divan::bench(args = ["rust", "python", "typescript"])]
fn highlight(bencher: Bencher<'_, '_>, language: &str) {
    let (_, code) = CODE.iter().find(|(name, _)| *name == language).unwrap();
    let grammars = Grammars::default();
    grammars.highlight(language, code).unwrap();
    bencher.bench(|| grammars.highlight(black_box(language), black_box(code)).unwrap().len());
}
