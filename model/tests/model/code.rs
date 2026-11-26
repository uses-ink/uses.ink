use model::code::{self, Code, Keyed};
use model::decision::{self, File, Mismatch, Scope, Site};
use model::{
    AnyCode, AnyDecision, ContentPath, Decided, DecisionCode, DefaultReason, Evidence, Rejection, Span, Subject, Tag,
};

fn path(text: &str) -> ContentPath {
    ContentPath::parse(text).unwrap()
}

fn pinned() -> Evidence {
    Evidence::Config { span: Span::new(3, 9).unwrap() }
}

fn title() -> decision::Decision<code::Title> {
    let front = Evidence::Frontmatter { path: path("posts/a.md"), span: Span::new(10, 15).unwrap() };
    decision::Decision::new(File(path("posts/a.md")), "Pinned".to_owned(), pinned())
        .rejecting(decision::Alternative::new("Written".to_owned(), front, Rejection::Outranked))
}

/// What a generic consumer sees of a code: its name and scope, from the type alone.
fn described<C: Code>() -> (&'static str, Scope) {
    (C::NAME, <C::Subject as decision::Subject>::SCOPE)
}

#[test]
fn every_code_is_the_untyped_code_of_the_same_name_and_scope() {
    let typed: Vec<(&str, Scope)> = AnyCode::ALL.iter().map(|code| (code.name(), code.scope())).collect();
    let untyped: Vec<(&str, Scope)> = DecisionCode::ALL
        .iter()
        .map(|code| (code.name(), if code.is_sitewide() { Scope::Site } else { Scope::File }))
        .collect();
    assert_eq!(typed, untyped);
    for code in AnyCode::ALL {
        assert_eq!(AnyCode::from(DecisionCode::from(*code)), *code);
        assert_eq!(code.name().parse::<AnyCode>(), Ok(*code));
        assert_eq!(code.to_string(), code.name());
    }
    assert!("page.nope".parse::<AnyCode>().is_err());
}

#[test]
fn a_code_type_carries_its_name_scope_and_keys() {
    assert_eq!(described::<code::Title>(), ("page.title", Scope::File));
    assert_eq!(described::<code::Home>(), ("site.home", Scope::Site));
    assert_eq!(code::Summary::CODE, AnyCode::Summary);
    assert_eq!(<code::Summary as Keyed>::KEYS, ["description", "summary"]);
    assert_eq!(<code::Kind as Keyed>::KEYS, ["type"]);
}

#[test]
fn a_typed_decision_erases_and_comes_back() {
    let erased = AnyDecision::from(title());
    assert_eq!(erased.code(), AnyCode::Title);
    assert_eq!(erased.subject(), Subject::File(path("posts/a.md")));
    assert_eq!(erased.evidence(), &pinned());
    assert_eq!(erased.get::<code::Title>(), Some(&title()));
    assert_eq!(erased.get::<code::SiteTitle>(), None);
    assert_eq!(decision::Decision::<code::Title>::try_from(erased.clone()), Ok(title()));
    assert_eq!(
        decision::Decision::<code::Date>::try_from(erased),
        Err(Mismatch::Code { expected: AnyCode::Date, found: AnyCode::Title })
    );
}

#[test]
fn an_erased_decision_is_written_with_its_code_as_tag() {
    let erased = AnyDecision::from(title());
    let json = serde_json::to_value(&erased).unwrap();
    assert_eq!(json["code"], "page.title");
    assert_eq!(json["subject"], serde_json::json!({ "file": "posts/a.md" }));
    assert_eq!(json["value"], "Pinned");
    assert_eq!(json["rejected"][0]["because"], "outranked");
    assert_eq!(serde_json::from_value::<AnyDecision>(json).unwrap(), erased);

    let home = AnyDecision::from(decision::Decision::<code::Home>::new(
        Site,
        None,
        Evidence::Default { reason: DefaultReason::NothingMatched },
    ));
    let json = serde_json::to_value(&home).unwrap();
    assert_eq!(json["subject"], "site");
    assert_eq!(serde_json::from_value::<AnyDecision>(json).unwrap(), home);
}

#[test]
fn an_erased_decision_refuses_a_subject_of_the_other_scope() {
    let json = serde_json::json!({
        "code": "site.home", "subject": { "file": "a.md" }, "value": null,
        "evidence": { "config": { "span": [0, 1] } }, "rejected": [],
    });
    assert!(serde_json::from_value::<AnyDecision>(json).is_err());
}

#[test]
fn a_stored_tag_is_checked_against_its_text() {
    let tag = Tag::new("Rust Notes").unwrap();
    let json = serde_json::to_value(&tag).unwrap();
    assert_eq!(serde_json::from_value::<Tag>(json).unwrap(), tag);
    let stale = serde_json::json!({ "slug": "other", "written": "Rust Notes" });
    assert!(serde_json::from_value::<Tag>(stale).is_err());
}

#[test]
fn untyped_decisions_convert_without_loss() {
    let tags = vec![Tag::new("rust").unwrap()];
    let rejected = model::Alternative::new(Decided::Tags(vec![]), pinned(), Rejection::OnlyBound);
    let untyped = [
        model::Decision::new(Decided::Title("T".to_owned()), Subject::File(path("a.md")), pinned()),
        model::Decision::new(Decided::Tags(tags), Subject::File(path("a.md")), pinned()).rejecting(rejected),
        model::Decision::new(Decided::Home(None), Subject::Site, pinned()),
        model::Decision::new(Decided::Root(model::Root::Top), Subject::Site, pinned()),
    ];
    for old in untyped {
        let erased = AnyDecision::try_from(old.clone()).unwrap();
        assert_eq!(erased.code(), AnyCode::from(old.code()));
        assert_eq!(model::Decision::from(erased), old);
    }
    let typed = AnyDecision::from(title());
    assert_eq!(AnyDecision::try_from(model::Decision::from(typed.clone())), Ok(typed));
}

#[test]
fn an_untyped_decision_that_mixes_codes_or_scopes_is_refused() {
    let on_site = model::Decision::new(Decided::Title("T".to_owned()), Subject::Site, pinned());
    assert_eq!(AnyDecision::try_from(on_site), Err(Mismatch::Scope { expected: Scope::File, found: Scope::Site }));
    let mixed = model::Decision::new(Decided::Title("T".to_owned()), Subject::File(path("a.md")), pinned())
        .rejecting(model::Alternative::new(Decided::Publish(true), pinned(), Rejection::Outranked));
    let refused = AnyDecision::try_from(mixed).unwrap_err();
    assert_eq!(refused, Mismatch::Code { expected: AnyCode::Title, found: AnyCode::Publish });
    assert_eq!(refused.to_string(), "the decision is `page.publish`, not `page.title`");
}

#[test]
fn values_show_as_a_person_reads_them() {
    use model::Value;
    assert_eq!("Hello".to_owned().shown().to_string(), "“Hello”");
    assert_eq!(None::<String>.shown().to_string(), "none");
    let tags = vec![Tag::new("rust").unwrap(), Tag::new("Notes").unwrap()];
    assert_eq!(tags.shown().to_string(), "rust, Notes");
    assert_eq!(Vec::<Tag>::new().shown().to_string(), "none");
    assert_eq!(model::PageKind::Post.shown().to_string(), "a post");
}

#[test]
fn a_code_words_its_values_not_its_value_type() {
    use model::Worded;
    assert_eq!(Worded::<code::Date>::new(&None).to_string(), "undated");
    assert_eq!(Worded::<code::Updated>::new(&None).to_string(), "never updated");
    assert_eq!(Worded::<code::Summary>::new(&None).to_string(), "none");
    assert_eq!(Worded::<code::Publish>::new(&false).to_string(), "not published");
    assert_eq!(Worded::<code::Ignored>::new(&false).to_string(), "kept");
    assert_eq!(Worded::<code::Tags>::new(&vec![]).to_string(), "no tags");
    assert_eq!(Worded::<code::Nav>::new(&vec![path("a.md"), path("b.md")]).to_string(), "a.md, b.md");
    assert_eq!(Worded::<code::Title>::new(&"Hi".to_owned()).to_string(), "“Hi”");
    assert_eq!(<code::Home as model::Word>::NONE, "a generated listing");
}

/// A stand-in consumer of [`model::Each`]: the name and scope of the code it is run for.
struct Describe;

impl<C: Code> model::Each<C> for Describe {
    type Out = (&'static str, Scope);

    fn each(&mut self) -> Self::Out {
        described::<C>()
    }
}

#[test]
fn each_runs_the_work_of_the_code_it_names() {
    for code in AnyCode::ALL {
        assert_eq!(code.each(&mut Describe), (code.name(), code.scope()));
    }
}

/// A stand-in family: each code's value type.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Values;

impl model::Family for Values {
    type Of<C: Code> = C::Value;
}

#[test]
fn any_of_holds_one_code_and_gives_back_only_that_one() {
    let title = model::AnyOf::<Values>::new::<code::Title>("Pinned".to_owned());
    assert_eq!(title.code(), AnyCode::Title);
    assert_eq!(title.get::<code::Title>().map(String::as_str), Some("Pinned"));
    assert_eq!(title.get::<code::SiteTitle>(), None);
}
