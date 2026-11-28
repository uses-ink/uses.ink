use std::error::Error as _;

use model::{Form, FromFront, PageKind, Raw, Slug, Span, Tag, Wanted, When};

fn span(start: usize, end: usize) -> Span {
    Span::new(start, end).unwrap()
}

fn text(value: &str, at: usize) -> Raw<'_> {
    Raw::Text { value: value.into(), span: span(at, at + value.len()) }
}

#[test]
fn text_reads_numbers_and_flags_as_written() {
    let number = Raw::Number { written: "1.10", span: span(0, 4) };
    assert_eq!(String::read(&number).unwrap(), "1.10");
    let flag = Raw::Bool { value: true, written: "true", span: span(0, 4) };
    assert_eq!(String::read(&flag).unwrap(), "true");
    assert_eq!(String::read(&text("  Hello  ", 0)).unwrap(), "Hello");
    let blank = String::read(&text("   ", 7)).unwrap_err();
    assert_eq!((blank.first().wanted, blank.first().span), (Wanted::Text, span(7, 10)));
}

#[test]
fn a_flag_is_only_a_boolean() {
    assert!(bool::read(&Raw::Bool { value: false, written: "false", span: span(0, 5) }).is_ok_and(|flag| !flag));
    let quoted = bool::read(&text("true", 2)).unwrap_err();
    assert_eq!(quoted.first().wanted, Wanted::Flag);
    assert_eq!(quoted.to_string(), "`true` or `false`");
}

#[test]
fn an_option_reads_null_and_false_as_none() {
    assert_eq!(Option::<When>::read(&Raw::Null { span: span(0, 0) }).unwrap(), None);
    let undated = Raw::Bool { value: false, written: "false", span: span(0, 5) };
    assert_eq!(Option::<When>::read(&undated).unwrap(), None);
    assert!(Option::<When>::read(&text("2024-03-01", 0)).unwrap().is_some());
    let wrong = Option::<When>::read(&Raw::Bool { value: true, written: "true", span: span(0, 4) }).unwrap_err();
    assert_eq!(wrong.first().wanted, Wanted::When);
}

#[test]
fn a_refused_day_keeps_its_cause() {
    let refused = When::read(&text("2024-02-30", 6)).unwrap_err();
    let expected = refused.first();
    assert_eq!(expected.span, span(6, 16));
    let cause = expected.source().unwrap();
    assert_eq!(cause.to_string(), "the text is no calendar day");
    assert!(cause.source().is_some(), "the calendar's own error is kept");
}

#[test]
fn a_list_reads_each_item_and_refuses_every_bad_one() {
    let list = Raw::List { items: vec![text("rust", 1), text("!!", 8), text("?", 12)], span: span(0, 13) };
    let refused = Vec::<Tag>::read(&list).unwrap_err();
    let spans: Vec<Span> = refused.iter().map(|expected| expected.span).collect();
    assert_eq!(spans, [span(8, 10), span(12, 13)]);
    let good = Raw::List { items: vec![text("rust", 1), text("Notes", 8)], span: span(0, 13) };
    let tags = Vec::<Tag>::read(&good).unwrap();
    assert_eq!(tags.iter().map(|tag| tag.written.as_str()).collect::<Vec<_>>(), ["rust", "Notes"]);
}

#[test]
fn tags_also_read_from_text_separated_by_commas() {
    let tags = Vec::<Tag>::read(&text("rust, notes,,", 0)).unwrap();
    assert_eq!(tags, [Tag::new("rust").unwrap(), Tag::new("notes").unwrap()]);
    assert_eq!(Vec::<Tag>::read(&Raw::Null { span: span(0, 0) }).unwrap(), []);
}

#[test]
fn a_mapping_where_a_list_is_wanted_says_so() {
    let refused = Vec::<Tag>::read(&Raw::Map { span: span(4, 20) }).unwrap_err();
    assert_eq!(refused.first().form, Form::List);
    assert_eq!(refused.to_string(), "a list, each item a tag with a letter or digit");
}

#[test]
fn a_slug_is_read_strictly_and_a_kind_in_any_case() {
    assert_eq!(Slug::read(&text("hello-world", 0)).unwrap().as_str(), "hello-world");
    let refused = Slug::read(&text("Hello World", 0)).unwrap_err();
    assert!(refused.first().cause.is_some());
    assert_eq!(PageKind::read(&text("Post", 0)).unwrap(), PageKind::Post);
    assert_eq!(PageKind::read(&text("home", 0)).unwrap_err().first().wanted, Wanted::Kind);
}
