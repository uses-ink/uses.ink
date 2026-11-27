//! `Config::parse(&pins(decisions))` pins every decision's value, with no diagnostics, and
//! `pins_into` an empty file reads back the same. Every code is reached through `AnyCode::each`,
//! so a code added to `model` does not build here until its value type has a strategy and a
//! sample, and does not build in config until it has a `Toml` impl.

use std::collections::BTreeMap;

use jiff::civil;
use model::code::Code;
use model::decision::{self, File, Site};
use model::record::PluginId;
use model::{
    AnyCode, AnyDecision, Author, ContentPath, DefaultReason, Each, Evidence, Language, Layout, PageKind, Problem,
    Root, Slug, Subject, Tag, Timestamp, When,
};
use proptest::prelude::*;
use uses_ink_config::{Config, pins, pins_into};

use crate::Pins;

/// A value type the tests can make: any value, and one written by hand.
trait Arbitrary: Sized {
    fn any() -> BoxedStrategy<Self>;
    fn sample() -> Self;
}

impl<T: Arbitrary + std::fmt::Debug + 'static> Arbitrary for Option<T> {
    fn any() -> BoxedStrategy<Self> {
        prop::option::of(T::any()).boxed()
    }

    fn sample() -> Self {
        Some(T::sample())
    }
}

impl<T: Arbitrary + std::fmt::Debug + 'static> Arbitrary for Vec<T> {
    fn any() -> BoxedStrategy<Self> {
        prop::collection::vec(T::any(), 0..4).boxed()
    }

    fn sample() -> Self {
        vec![T::sample()]
    }
}

impl Arbitrary for ContentPath {
    fn any() -> BoxedStrategy<Self> {
        prop::collection::vec("[^/\\\\\0]{1,6}", 1..4)
            .prop_filter("no `.` or `..` part", |parts| parts.iter().all(|p| p != "." && p != ".."))
            .prop_map(|parts| Self::parse(parts.join("/")).unwrap())
            .boxed()
    }

    fn sample() -> Self {
        Self::parse("posts/a.md").unwrap()
    }
}

impl Arbitrary for When {
    /// Years TOML writes, 0 to 9999.
    fn any() -> BoxedStrategy<Self> {
        prop_oneof![
            (0_i16..=9999, 1_i8..=12, 1_i8..=28).prop_map(|(y, m, d)| Self::Day(civil::date(y, m, d))),
            (-62_167_219_200_i64..253_402_300_800, 0_i32..1_000_000_000)
                .prop_map(|(seconds, nanos)| Self::Instant(Timestamp::new(seconds, nanos).unwrap())),
        ]
        .boxed()
    }

    fn sample() -> Self {
        Self::Day(civil::date(2024, 3, 1))
    }
}

impl Arbitrary for Root {
    fn any() -> BoxedStrategy<Self> {
        prop_oneof![Just(Self::Top), ContentPath::any().prop_map(Self::Folder)].boxed()
    }

    fn sample() -> Self {
        Self::Folder(ContentPath::sample())
    }
}

impl Arbitrary for Layout {
    fn any() -> BoxedStrategy<Self> {
        prop::sample::select(vec![Self::Blog, Self::Vault, Self::Docs, Self::Flat]).boxed()
    }

    fn sample() -> Self {
        Self::Vault
    }
}

impl Arbitrary for PageKind {
    fn any() -> BoxedStrategy<Self> {
        prop::sample::select(vec![Self::Home, Self::Post, Self::Page]).boxed()
    }

    fn sample() -> Self {
        Self::Post
    }
}

impl Arbitrary for bool {
    fn any() -> BoxedStrategy<Self> {
        proptest::arbitrary::any::<Self>().boxed()
    }

    fn sample() -> Self {
        true
    }
}

impl Arbitrary for String {
    fn any() -> BoxedStrategy<Self> {
        proptest::arbitrary::any::<Self>().boxed()
    }

    fn sample() -> Self {
        "On shelves".to_owned()
    }
}

impl Arbitrary for Slug {
    fn any() -> BoxedStrategy<Self> {
        "[a-z0-9]{1,5}(-[a-z0-9]{1,5}){0,2}".prop_map(|text| Self::parse(text).unwrap()).boxed()
    }

    fn sample() -> Self {
        Self::parse("on-shelves").unwrap()
    }
}

impl Arbitrary for Tag {
    fn any() -> BoxedStrategy<Self> {
        "[\\pL\\pN][\\PC]{0,6}".prop_map(|text| Self::new(text).unwrap()).boxed()
    }

    fn sample() -> Self {
        Self::new("Rust").unwrap()
    }
}

impl Arbitrary for Author {
    fn any() -> BoxedStrategy<Self> {
        "\\s?[\\pL\\pN][\\PC]{0,6}".prop_map(|name| Self::try_from(name).unwrap()).boxed()
    }

    fn sample() -> Self {
        Self::try_from("Ann".to_owned()).unwrap()
    }
}

impl Arbitrary for Language {
    fn any() -> BoxedStrategy<Self> {
        "[a-zA-Z]{2,3}(-[a-zA-Z0-9]{1,8}){0,2}".prop_map(|tag| tag.parse().unwrap()).boxed()
    }

    fn sample() -> Self {
        "fr-CH".parse().unwrap()
    }
}

impl Arbitrary for PluginId {
    /// A scope and a name of a format, in kebab-case.
    fn any() -> BoxedStrategy<Self> {
        "[a-z0-9]{1,4}(-[a-z0-9]{1,4}){0,1}:format(-[a-z0-9]{1,4}){1,2}".prop_map(|id| id.parse().unwrap()).boxed()
    }

    fn sample() -> Self {
        Self::obsidian()
    }
}

impl Arbitrary for Site {
    fn any() -> BoxedStrategy<Self> {
        Just(Self).boxed()
    }

    fn sample() -> Self {
        Self
    }
}

impl Arbitrary for File {
    /// Few files, so several codes land on one.
    fn any() -> BoxedStrategy<Self> {
        prop_oneof![ContentPath::any(), Just(ContentPath::sample())].prop_map(Self).boxed()
    }

    fn sample() -> Self {
        Self(ContentPath::parse("x.md").unwrap())
    }
}

fn nothing() -> Evidence {
    Evidence::Default { reason: DefaultReason::NothingMatched }
}

/// Any decision of the code it is run for.
struct Decisions;

impl<C: Code<Value: Arbitrary, Subject: Arbitrary>> Each<C> for Decisions {
    type Out = BoxedStrategy<AnyDecision>;

    fn each(&mut self) -> Self::Out {
        (C::Subject::any(), C::Value::any())
            .prop_map(|(subject, value)| decision::Decision::<C>::new(subject, value, nothing()).into())
            .boxed()
    }
}

/// The sample decision of the code it is run for, and a pin of it with a table for its value.
struct Samples;

impl<C: Code<Value: Arbitrary, Subject: Arbitrary>> Each<C> for Samples {
    type Out = (AnyDecision, String);

    fn each(&mut self) -> Self::Out {
        let decision = decision::Decision::<C>::new(C::Subject::sample(), C::Value::sample(), nothing());
        let (table, key) = C::NAME.split_once('.').unwrap();
        let wrong = match decision.subject.clone().into() {
            Subject::Site => format!("[{table}]\n{key} = {{}}\n"),
            _ => format!("[{table}.\"x.md\"]\n{key} = {{}}\n"),
        };
        (decision.into(), wrong)
    }
}

/// Each code is written as a pin, reads back, and refuses a value of the wrong shape with one
/// diagnostic of its own.
#[test]
fn every_code() {
    for code in AnyCode::ALL {
        let (decision, wrong) = code.each(&mut Samples);
        let written = pins(std::slice::from_ref(&decision));
        assert!(!written.contains("not pinned"), "`{code}` is not written:\n{written}");
        reads_back(&written, std::slice::from_ref(&decision)).unwrap();

        let diagnostics = Config::parse(&wrong).diagnostics;
        let worded = matches!(
            diagnostics.as_slice(),
            [diagnostic] if matches!(&diagnostic.problem, Problem::ConfigValueInvalid { expected, .. } if !expected.is_empty())
        );
        assert!(worded, "`{code}` is not refused with its shape: {diagnostics:?}");
    }
}

fn decision() -> impl Strategy<Value = AnyDecision> {
    prop::sample::select(AnyCode::ALL.to_vec()).prop_flat_map(|code| code.each(&mut Decisions))
}

/// One decision per code and subject, as an engine gives them.
fn decisions() -> impl Strategy<Value = Vec<AnyDecision>> {
    prop::collection::vec(decision(), 0..24).prop_map(|all| {
        let unique: BTreeMap<(AnyCode, Subject), AnyDecision> =
            all.into_iter().map(|d| ((d.code(), d.subject()), d)).collect();
        unique.into_values().collect()
    })
}

fn reads_back(text: &str, decisions: &[AnyDecision]) -> Result<(), TestCaseError> {
    let parsed = Config::parse(text);
    prop_assert_eq!(&parsed.diagnostics, &[], "{}", text);
    for decision in decisions {
        let pinned = decision.code().each(&mut Pins { config: &parsed.config, decision });
        prop_assert!(pinned, "{:?} is not pinned by\n{}", decision, text);
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
