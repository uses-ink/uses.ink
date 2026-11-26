//! Every `fixture/site/<name>/` folder interpreted and rendered, compared through the shared
//! harness with `fixture/site/<name>.snap`: the decisions, the diagnostics and notices, the
//! manifest, and the bytes of every text entry.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use engine::{Engine, NoPins};

use crate::support::{Pinned, Plain, Tree, settings};

const SITES: &str = "fixture/site";

/// Every file under `dir`, as `(path below the site, text)`.
fn files(dir: &Path, base: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, base, out);
        } else {
            let relative = path.strip_prefix(base).unwrap().to_str().unwrap().to_owned();
            out.push((relative, std::fs::read_to_string(&path).unwrap()));
        }
    }
}

fn on(subject: Option<&model::Subject>) -> String {
    subject.map_or_else(|| "nothing".to_owned(), ToString::to_string)
}

fn report(site: &Path) -> String {
    let mut found = Vec::new();
    files(site, site, &mut found);
    let tree = found.iter().fold(Tree::new(), |tree, (path, text)| tree.file(path, text));
    let understood = tree.understand(&Pinned::none());
    let mut out = String::new();
    let _infallible = writeln!(out, "# decisions");
    for decision in &understood.decisions {
        let _infallible = writeln!(out, "{}", serde_json::to_string(decision).unwrap());
    }
    let _infallible = writeln!(out, "# diagnostics and notices");
    for diagnostic in &understood.diagnostics {
        let _infallible = writeln!(out, "{} on {}", diagnostic.code(), on(diagnostic.subject.as_ref()));
    }
    for notice in &understood.notices {
        let _infallible = writeln!(out, "{} on {}", notice.code(), on(notice.subject.as_ref()));
    }
    let Some(site) = understood.site else { return out };
    let rendered = Engine::new((), NoPins, (), Plain::default()).render(&site, &settings());
    let _infallible = writeln!(out, "# deploy {}", rendered.manifest.id());
    for entry in rendered.manifest.entries() {
        let _infallible =
            writeln!(out, "{} {} {} {}", entry.route(), entry.status().code(), entry.media(), entry.blob());
    }
    for entry in rendered.manifest.entries().iter().filter(|e| e.media().is_text()) {
        if let Some(bytes) = rendered.blobs.get(&entry.blob()) {
            let _infallible = writeln!(out, "## {}\n{}", entry.route(), String::from_utf8_lossy(bytes));
        }
    }
    out
}

#[test]
fn sites_render_as_expected() {
    let mut sites: Vec<PathBuf> =
        std::fs::read_dir(SITES).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    sites.sort();
    let cases = sites.iter().map(|site| (site.file_name().unwrap().to_str().unwrap().to_owned(), report(site)));
    fixture::cases(SITES, cases);
}
