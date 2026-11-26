//! `Config::parse(&pins(decisions))` pins every decision's value, with no diagnostics, and
//! `pins_into` an empty file reads back the same. `every_code` checks each code of
//! [`DecisionCode::ALL`] once, so a code added to `model` fails here until `config` reads,
//! writes and words it.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use std::collections::BTreeMap;

use jiff::civil;
use model::{
    Author, ContentPath, Decided, Decision, DecisionCode, DefaultReason, Evidence, Language, Layout, PageKind, Problem,
    Root, Slug, Subject, Tag, Timestamp, When,
};
use proptest::prelude::*;
use uses_ink_config::{Config, pins, pins_into};

fn path() -> impl Strategy<Value = ContentPath> {
    prop::collection::vec("[^/\\\\\0]{1,6}", 1..4)
        .prop_filter("no `.` or `..` part", |parts| parts.iter().all(|p| p != "." && p != ".."))
        .prop_map(|parts| ContentPath::parse(parts.join("/")).unwrap())
}

/// Years TOML writes, 0 to 9999.
fn when() -> impl Strategy<Value = When> {
    prop_oneof![
        (0_i16..=9999, 1_i8..=12, 1_i8..=28).prop_map(|(y, m, d)| When::Day(civil::date(y, m, d))),
        (-62_167_219_200_i64..253_402_300_800, 0_i32..1_000_000_000)
            .prop_map(|(seconds, nanos)| When::Instant(Timestamp::new(seconds, nanos).unwrap())),
    ]
}

fn decided(code: DecisionCode) -> BoxedStrategy<Decided> {
    match code {
        DecisionCode::Root => {
            prop_oneof![Just(Root::Top), path().prop_map(Root::Folder)].prop_map(Decided::Root).boxed()
        }
        DecisionCode::Layout => prop::sample::select(vec![Layout::Blog, Layout::Vault, Layout::Docs, Layout::Flat])
            .prop_map(Decided::Layout)
            .boxed(),
        DecisionCode::Ignored => any::<bool>().prop_map(Decided::Ignored).boxed(),
        DecisionCode::Home => prop::option::of(path()).prop_map(Decided::Home).boxed(),
        DecisionCode::Kind => {
            prop::sample::select(vec![PageKind::Home, PageKind::Post, PageKind::Page]).prop_map(Decided::Kind).boxed()
        }
        DecisionCode::Publish => any::<bool>().prop_map(Decided::Publish).boxed(),
        DecisionCode::Title => any::<String>().prop_map(Decided::Title).boxed(),
        DecisionCode::Date => prop::option::of(when()).prop_map(Decided::Date).boxed(),
        DecisionCode::Updated => prop::option::of(when()).prop_map(Decided::Updated).boxed(),
        DecisionCode::Slug => {
            "[a-z0-9]{1,5}(-[a-z0-9]{1,5}){0,2}".prop_map(|text| Decided::Slug(Slug::parse(text).unwrap())).boxed()
        }
        DecisionCode::Tags => prop::collection::vec("[\\pL\\pN][\\PC]{0,6}", 0..4)
            .prop_map(|tags| Decided::Tags(tags.into_iter().map(|t| Tag::new(t).unwrap()).collect()))
            .boxed(),
        DecisionCode::Nav => prop::collection::vec(path(), 0..4).prop_map(Decided::Nav).boxed(),
        DecisionCode::Summary => prop::option::of(any::<String>()).prop_map(Decided::Summary).boxed(),
        DecisionCode::SiteTitle => any::<String>().prop_map(Decided::SiteTitle).boxed(),
        DecisionCode::Description => prop::option::of(any::<String>()).prop_map(Decided::Description).boxed(),
        DecisionCode::Authors => prop::collection::vec("\\s?[\\pL\\pN][\\PC]{0,6}", 0..4)
            .prop_map(|names| Decided::Authors(names.into_iter().map(|n| Author::try_from(n).unwrap()).collect()))
            .boxed(),
        DecisionCode::Language => "[a-zA-Z]{2,3}(-[a-zA-Z0-9]{1,8}){0,2}"
            .prop_map(|tag| Decided::Language(tag.parse::<Language>().unwrap()))
            .boxed(),
        _ => unreachable!("a code this test does not know"),
    }
}

/// One value of `code`, written by hand so a new code has to be added here.
fn sample(code: DecisionCode) -> Option<Decided> {
    let path = || ContentPath::parse("posts/a.md").unwrap();
    match code {
        DecisionCode::Root => Some(Decided::Root(Root::Folder(path()))),
        DecisionCode::Layout => Some(Decided::Layout(Layout::Vault)),
        DecisionCode::Ignored => Some(Decided::Ignored(true)),
        DecisionCode::Home => Some(Decided::Home(None)),
        DecisionCode::Kind => Some(Decided::Kind(PageKind::Post)),
        DecisionCode::Publish => Some(Decided::Publish(false)),
        DecisionCode::Title => Some(Decided::Title("On shelves".to_owned())),
        DecisionCode::Date => Some(Decided::Date(Some(When::Day(civil::date(2024, 3, 1))))),
        DecisionCode::Updated => Some(Decided::Updated(None)),
        DecisionCode::Slug => Some(Decided::Slug(Slug::parse("on-shelves").unwrap())),
        DecisionCode::Tags => Some(Decided::Tags(vec![Tag::new("Rust").unwrap()])),
        DecisionCode::Nav => Some(Decided::Nav(vec![path()])),
        DecisionCode::Summary => Some(Decided::Summary(Some("Books, and where they go.".to_owned()))),
        DecisionCode::SiteTitle => Some(Decided::SiteTitle("Ann's notes".to_owned())),
        DecisionCode::Description => Some(Decided::Description(None)),
        DecisionCode::Authors => Some(Decided::Authors(vec![Author::try_from("Ann".to_owned()).unwrap()])),
        DecisionCode::Language => Some(Decided::Language("fr-CH".parse().unwrap())),
        _ => None,
    }
}

/// The subject a code is decided on.
fn subject(code: DecisionCode) -> Subject {
    if code.is_sitewide() { Subject::Site } else { Subject::File(ContentPath::parse("x.md").unwrap()) }
}

/// Each code is written as a pin, reads back, and has its own wording when its value is wrong.
#[test]
fn every_code() {
    for code in DecisionCode::ALL {
        let sampled = sample(*code);
        assert!(sampled.is_some(), "no sample of `{code}`: add one, and teach config to read and write it");
        let decision = Decision::new(
            sampled.unwrap(),
            subject(*code),
            Evidence::Default { reason: DefaultReason::NothingMatched },
        );
        let written = pins(std::slice::from_ref(&decision));
        assert!(!written.contains("not pinned"), "`{code}` is not written:\n{written}");
        reads_back(&written, std::slice::from_ref(&decision)).unwrap();

        let (table, key) = code.name().split_once('.').unwrap();
        let wrong = match decision.subject {
            Subject::Site => format!("[{table}]\n{key} = {{}}\n"),
            _ => format!("[{table}.\"x.md\"]\n{key} = {{}}\n"),
        };
        let diagnostics = Config::parse(&wrong).diagnostics;
        let worded = matches!(
            diagnostics.as_slice(),
            [diagnostic] if matches!(
                &diagnostic.problem,
                Problem::ConfigValueInvalid { expected, .. } if !expected.contains("does not read")
            )
        );
        assert!(worded, "`{code}` has no wording of its own: {diagnostics:?}");
    }
}

fn decision() -> impl Strategy<Value = Decision> {
    prop::sample::select(DecisionCode::ALL.to_vec())
        .prop_flat_map(|code| {
            let subject = if code.is_sitewide() {
                Just(Subject::Site).boxed()
            } else {
                // few files, so several codes land on one
                prop_oneof![path(), Just(ContentPath::parse("posts/a.md").unwrap())].prop_map(Subject::File).boxed()
            };
            (decided(code), subject)
        })
        .prop_map(|(decided, subject)| {
            Decision::new(decided, subject, Evidence::Default { reason: DefaultReason::NothingMatched })
        })
}

/// One decision per code and subject, as an engine gives them.
fn decisions() -> impl Strategy<Value = Vec<Decision>> {
    prop::collection::vec(decision(), 0..24).prop_map(|all| {
        let unique: BTreeMap<(DecisionCode, Subject), Decision> =
            all.into_iter().map(|d| ((d.code(), d.subject.clone()), d)).collect();
        unique.into_values().collect()
    })
}

fn reads_back(text: &str, decisions: &[Decision]) -> Result<(), TestCaseError> {
    let parsed = Config::parse(text);
    prop_assert_eq!(&parsed.diagnostics, &[], "{}", text);
    for decision in decisions {
        let pin = parsed.config.pin(decision.code(), &decision.subject);
        prop_assert_eq!(pin.map(|p| &p.decided), Some(&decision.decided), "{}", text);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn pins_read_back(decisions in decisions()) {
        reads_back(&pins(&decisions), &decisions)?;
        reads_back(&pins_into("", &decisions).unwrap(), &decisions)?;
    }
}
