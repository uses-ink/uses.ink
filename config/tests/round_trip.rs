//! `Config::parse(&pins(decisions))` pins every decision's value, with no diagnostics, and
//! `pins_into` an empty file reads back the same.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use std::collections::BTreeMap;

use jiff::civil;
use model::{
    ContentPath, Decided, Decision, DecisionCode, DefaultReason, Evidence, Layout, PageKind, Root, Slug, Subject, Tag,
    Timestamp, When,
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
        _ => unreachable!("a code this test does not know"),
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
