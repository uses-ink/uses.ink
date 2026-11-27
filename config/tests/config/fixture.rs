//! The fixtures under `fixture/`: `parse/<name>.toml` read with `Config::parse` and printed,
//! `pin/<name>.toml` with the decisions below written into it by `pins_into`, and `pins.snap`,
//! the same decisions written by `pins`. The shared harness compares each with its golden.

use std::fmt::Write as _;

use jiff::civil::date;
use model::code::{self, Code};
use model::decision::{self, File, Site};
use model::{
    AnyCode, AnyDecision, Author, ContentPath, DefaultReason, Each, Evidence, Layout, PageKind, Root, Slug, Span,
    Subject, Tag, Timestamp, When,
};
use uses_ink_config::{Config, Listing, Parsed, Pattern, include_into, pins, pins_into};

use crate::Pins;

/// The text a span covers, with newlines shown as `\n`.
fn quote(text: &str, span: Span) -> String {
    let covered = text.get(span.range()).unwrap_or("<out of range>").replace('\n', "\\n");
    format!("at {}..{} `{covered}`", span.start(), span.end())
}

/// Prints the pin of each code it is run for on one subject.
struct Print<'a> {
    config: &'a Config,
    subject: &'a Subject,
    text: &'a str,
    out: &'a mut String,
}

impl<C: Code> Each<C> for Print<'_> {
    type Out = ();

    fn each(&mut self) {
        let Ok(subject) = C::Subject::try_from(self.subject.clone()) else { return };
        if let Some(pin) = self.config.pin::<C>(&subject) {
            let at = quote(self.text, pin.span);
            writeln!(self.out, "pin {} on {}: {:?}({:?}) {at}", C::NAME, self.subject, C::CODE, pin.value).unwrap();
        }
    }
}

fn print(text: &str, parsed: &Parsed) -> String {
    let mut out = String::new();
    let config = &parsed.config;
    let subjects = std::iter::once(Subject::Site).chain(config.pinned_files().cloned().map(Subject::File));
    for subject in subjects {
        for code in AnyCode::ALL {
            code.each(&mut Print { config, subject: &subject, text, out: &mut out });
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
    fixture::files("fixture/parse", "toml", |_, bytes| {
        let text = std::str::from_utf8(bytes).unwrap();
        print(text, &Config::parse(text))
    });
}

fn path(text: &str) -> ContentPath {
    ContentPath::parse(text).unwrap()
}

/// A decision of `C` with nothing to say for itself.
fn decided<C: Code>(subject: C::Subject, value: C::Value) -> AnyDecision {
    decision::Decision::<C>::new(subject, value, Evidence::Default { reason: DefaultReason::NothingMatched }).into()
}

/// One decision of every code, out of order, on a few files.
fn decisions() -> Vec<AnyDecision> {
    let post = || File(path("posts/2024-03-01-hello.md"));
    let about = || File(path("about.md"));
    vec![
        decided::<code::Title>(post(), "Hello, \"world\"".to_owned()),
        decided::<code::Nav>(Site, vec![path("about.md"), path("posts")]),
        decided::<code::Home>(Site, Some(path("README.md"))),
        decided::<code::Root>(Site, Root::Top),
        decided::<code::Layout>(Site, Layout::Blog),
        decided::<code::Kind>(post(), PageKind::Post),
        decided::<code::Date>(post(), Some(When::Day(date(2024, 3, 1)))),
        decided::<code::Updated>(post(), Some(When::Instant("2024-03-02T09:30:00.25Z".parse::<Timestamp>().unwrap()))),
        decided::<code::Slug>(post(), Slug::parse("hello").unwrap()),
        decided::<code::Tags>(post(), vec![Tag::new("Rust").unwrap(), Tag::new("notes on shelves").unwrap()]),
        decided::<code::Publish>(about(), false),
        decided::<code::Date>(about(), None),
        decided::<code::Ignored>(File(path("drafts/x.md")), true),
        decided::<code::Ignored>(File(path("LICENSE")), false),
        decided::<code::Summary>(post(), Some("Books, and where they go.".to_owned())),
        decided::<code::Summary>(about(), None),
        decided::<code::Language>(Site, "fr-CH".parse().unwrap()),
        decided::<code::Authors>(
            Site,
            vec![Author::try_from("Ann".to_owned()).unwrap(), Author::try_from("Bo Lin".to_owned()).unwrap()],
        ),
        decided::<code::Description>(Site, None),
        decided::<code::SiteTitle>(Site, "Ann's notes".to_owned()),
    ]
}

#[test]
fn pins_into_existing() {
    fixture::files("fixture/pin", "toml", |_, bytes| {
        match pins_into(std::str::from_utf8(bytes).unwrap(), &decisions()) {
            Ok(written) => written,
            Err(error) => format!("error: {error:?}\n"),
        }
    });
}

#[test]
fn pins_whole_file() {
    let mut all = decisions();
    all.push(decided::<code::Date>(File(path("old.md")), Some(When::Day(date(-5, 1, 1)))));
    let written = pins(&all);
    fixture::cases("fixture", [("pins", written.clone())]);
    let parsed = Config::parse(&written);
    assert_eq!(parsed.diagnostics, []);
    for decision in decisions() {
        assert!(decision.code().each(&mut Pins { config: &parsed.config, decision: &decision }), "{decision:?}");
    }
}

#[test]
fn include_is_added_and_removed_in_place() {
    fixture::files("fixture/include", "toml", |_, bytes| {
        let text = std::str::from_utf8(bytes).unwrap();
        let pattern: Pattern = "notes/**".parse().unwrap();
        let mut out = String::new();
        for listing in [Listing::Add, Listing::Remove] {
            match include_into(text, &pattern, listing) {
                Ok(included) => {
                    let reparsed = Config::parse(&included.text);
                    let listed = reparsed.config.include().iter().any(|listed| listed.value == pattern);
                    writeln!(out, "{listing:?}: {:?}, listed now: {listed}\n{}", included.listed, included.text).unwrap();
                }
                Err(error) => writeln!(out, "{listing:?}: error: {error:?}").unwrap(),
            }
        }
        out
    });
}
