//! The fixtures under `fixture/`: `parse/<name>.toml` read with `Config::parse` and printed,
//! `pin/<name>.toml` with the decisions below written into it by `pins_into`, and `pins.snap`,
//! the same decisions written by `pins`. Each is compared with its golden by `insta`;
//! `INSTA_UPDATE=always` rewrites them (and the run still fails, so read the diff).

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use jiff::civil::date;
use model::{
    ContentPath, Decided, Decision, DecisionCode, DefaultReason, Evidence, Layout, PageKind, Root, Slug, Subject, Tag,
    Timestamp, When,
};
use uses_ink_config::{Config, Parsed, pins, pins_into};

fn fixture(part: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture").join(part)
}

/// Every file in `fixture/<dir>` with `extension`, sorted.
fn files(dir: &str, extension: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(fixture(dir))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == extension))
        .collect();
    found.sort();
    found
}

fn snapshots(path: &Path) -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(path);
    settings.set_prepend_module_to_snapshot(false);
    settings.set_omit_expression(true);
    settings
}

/// Runs every `fixture/<dir>/<name>.toml` through `run` and compares it with `<name>.snap`. An
/// empty suite and a golden without an input fail; an input without a golden fails in `insta`.
fn suite(dir: &str, run: impl Fn(&str) -> String) {
    let inputs = files(dir, "toml");
    assert!(!inputs.is_empty(), "no fixtures in fixture/{dir}");
    for golden in files(dir, "snap") {
        assert!(golden.with_extension("toml").exists(), "{} has no input", golden.display());
    }
    for input in inputs {
        let name = input.file_stem().unwrap().to_str().unwrap().to_owned();
        let text = std::fs::read_to_string(&input).unwrap();
        let mut settings = snapshots(&fixture(dir));
        settings.set_input_file(&input);
        settings.bind(|| insta::assert_snapshot!(name, run(&text)));
    }
}

/// The text a span covers, with newlines shown as `\n`.
fn quote(text: &str, span: model::Span) -> String {
    let covered = text.get(span.range()).unwrap_or("<out of range>").replace('\n', "\\n");
    format!("at {}..{} `{covered}`", span.start(), span.end())
}

fn print(text: &str, parsed: &Parsed) -> String {
    let mut out = String::new();
    let config = &parsed.config;
    let subjects = std::iter::once(Subject::Site).chain(config.pinned_files().cloned().map(Subject::File));
    for subject in subjects {
        for code in DecisionCode::ALL {
            if let Some(pin) = config.pin(*code, &subject) {
                writeln!(out, "pin {code} on {subject}: {:?} {}", pin.decided, quote(text, pin.span)).unwrap();
            }
        }
    }
    if let Some(theme) = config.theme() {
        match &theme.plugin {
            Some(id) => writeln!(out, "theme {} {}", id.value, quote(text, id.span)).unwrap(),
            None => writeln!(out, "theme default").unwrap(),
        }
        for token in &theme.tokens {
            writeln!(out, "token {} = {:?} {}", token.name.value, token.value.value, quote(text, token.value.span))
                .unwrap();
        }
    }
    for (name, patterns) in [("include", config.include()), ("exclude", config.exclude())] {
        for pattern in patterns {
            writeln!(out, "{name} {} {}", pattern.value, quote(text, pattern.span)).unwrap();
        }
    }
    for diagnostic in &parsed.diagnostics {
        let at = diagnostic.span.map_or_else(|| "at no span".to_owned(), |span| quote(text, span));
        writeln!(out, "{} {}: {:?} {at}", diagnostic.severity().name(), diagnostic.code(), diagnostic.problem).unwrap();
    }
    out
}

#[test]
fn parse() {
    suite("parse", |text| print(text, &Config::parse(text)));
}

fn path(text: &str) -> ContentPath {
    ContentPath::parse(text).unwrap()
}

fn decision(decided: Decided, subject: Subject) -> Decision {
    Decision::new(decided, subject, Evidence::Default { reason: DefaultReason::NothingMatched })
}

/// One decision of every code, out of order, on a few files.
fn decisions() -> Vec<Decision> {
    let post = || Subject::File(path("posts/2024-03-01-hello.md"));
    let about = || Subject::File(path("about.md"));
    vec![
        decision(Decided::Title("Hello, \"world\"".to_owned()), post()),
        decision(Decided::Nav(vec![path("about.md"), path("posts")]), Subject::Site),
        decision(Decided::Home(Some(path("README.md"))), Subject::Site),
        decision(Decided::Root(Root::Top), Subject::Site),
        decision(Decided::Layout(Layout::Blog), Subject::Site),
        decision(Decided::Kind(PageKind::Post), post()),
        decision(Decided::Date(Some(When::Day(date(2024, 3, 1)))), post()),
        decision(
            Decided::Updated(Some(When::Instant("2024-03-02T09:30:00.25Z".parse::<Timestamp>().unwrap()))),
            post(),
        ),
        decision(Decided::Slug(Slug::parse("hello").unwrap()), post()),
        decision(Decided::Tags(vec![Tag::new("Rust").unwrap(), Tag::new("notes on shelves").unwrap()]), post()),
        decision(Decided::Publish(false), about()),
        decision(Decided::Date(None), about()),
        decision(Decided::Ignored(true), Subject::File(path("drafts/x.md"))),
        decision(Decided::Ignored(false), Subject::File(path("LICENSE"))),
    ]
}

#[test]
fn pins_into_existing() {
    suite("pin", |text| match pins_into(text, &decisions()) {
        Ok(written) => written,
        Err(error) => format!("error: {error:?}\n"),
    });
}

#[test]
fn pins_whole_file() {
    let mut all = decisions();
    all.push(decision(Decided::Date(Some(When::Day(date(-5, 1, 1)))), Subject::File(path("old.md"))));
    all.push(decision(Decided::Home(None), Subject::File(path("x.md"))));
    let written = pins(&all);
    snapshots(&fixture("")).bind(|| insta::assert_snapshot!("pins", written));
    let parsed = Config::parse(&written);
    assert_eq!(parsed.diagnostics, []);
    for decision in decisions() {
        let pin = parsed.config.pin(decision.code(), &decision.subject).unwrap();
        assert_eq!(pin.decided, decision.decided);
    }
}
