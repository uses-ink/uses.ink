use super::{Catalog, Flaw, Key, Kind, Part, Piece, Template};

fn parse(text: &str) -> Result<Catalog, Vec<Flaw>> {
    Catalog::try_from(toml::from_str::<toml::Table>(text).unwrap())
}

fn flaws(text: &str) -> Vec<Flaw> {
    parse(text).unwrap_err()
}

const GOOD: &str = r#"
[field]
written = "The text, as written."
candidates = "Every file it could mean."

[diagnostic."link.target.ambiguous"]
severity = "error"
fault = "author"
fields = { written = "text", candidates = "paths" }
message = "`{written}` could be {candidates} {{literally}}"
fix = "pick one"

[notice."site.home.generated"]
message = "a listing is the home page"
"#;

#[test]
fn reads_a_good_catalog() {
    let catalog = parse(GOOD).unwrap();
    let [entry] = catalog.diagnostics.as_slice() else { panic!("one diagnostic") };
    assert_eq!(entry.variant, "LinkTargetAmbiguous");
    assert_eq!(entry.fields.iter().map(|f| f.kind).collect::<Vec<_>>(), [Kind::Text, Kind::Paths]);
    assert_eq!(
        entry.fields.iter().map(|f| f.doc.as_str()).collect::<Vec<_>>(),
        ["The text, as written.", "Every file it could mean."]
    );
    let expected = Template(vec![
        Piece::Text("`".to_owned()),
        Piece::Field("written".to_owned()),
        Piece::Text("` could be ".to_owned()),
        Piece::Field("candidates".to_owned()),
        Piece::Text(" {literally}".to_owned()),
    ]);
    assert_eq!(entry.message, expected);
    assert_eq!(catalog.notices.len(), 1);
}

#[test]
fn reads_the_real_catalog() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../diag/catalog.toml")).unwrap();
    assert!(parse(&text).is_ok());
}

#[test]
fn refuses_an_unknown_section() {
    assert_eq!(flaws("[problem.\"a.b.c\"]\nmessage = \"x\""), [Flaw::Section { name: "problem".to_owned() }]);
}

#[test]
fn refuses_codes_that_are_not_three_kebab_words() {
    for code in ["link.unresolved", "a.b.c.d", "Link.target.x", "link.target.to_x", "link.-x.y", "link.1x.y"] {
        let text = format!("[notice.\"{code}\"]\nmessage = \"x\"");
        assert_eq!(flaws(&text), [Flaw::Code { code: code.to_owned() }], "{code}");
    }
}

#[test]
fn refuses_codes_that_collide_in_rust() {
    let text = "[notice.\"a.b-c.d\"]\nmessage = \"x\"\n[notice.\"a-b.c.d\"]\nmessage = \"x\"";
    assert_eq!(flaws(text), [Flaw::Collision { code: "a.b-c.d".to_owned(), other: "a-b.c.d".to_owned() }]);
}

#[test]
fn refuses_unknown_and_missing_keys() {
    let text = "[diagnostic.\"a.b.c\"]\nseverity = \"error\"\nmessage = \"x\"\nhint = \"y\"";
    let code = "a.b.c".to_owned();
    assert_eq!(
        flaws(text),
        [Flaw::Key { code: code.clone(), key: "hint".to_owned() }, Flaw::Missing { code, key: Key::Fault }]
    );
    let notice = "[notice.\"a.b.c\"]\nseverity = \"note\"\nmessage = \"x\"";
    assert_eq!(flaws(notice), [Flaw::Key { code: "a.b.c".to_owned(), key: "severity".to_owned() }]);
}

#[test]
fn refuses_values_of_the_wrong_kind() {
    let code = || "a.b.c".to_owned();
    assert_eq!(flaws("[notice.\"a.b.c\"]\nmessage = 3"), [Flaw::NotText { code: code(), key: Key::Message }]);
    let text = "[diagnostic.\"a.b.c\"]\nseverity = \"fatal\"\nfault = \"theirs\"\nmessage = \"x\"";
    assert_eq!(
        flaws(text),
        [
            Flaw::Severity { code: code(), given: "fatal".to_owned() },
            Flaw::Fault { code: code(), given: "theirs".to_owned() },
        ]
    );
}

#[test]
fn refuses_bad_fields() {
    let code = || "a.b.c".to_owned();
    let text = "[notice.\"a.b.c\"]\nfields = { when = \"date\", type = \"text\", Big = \"text\" }\nmessage = \"x\"";
    assert_eq!(
        flaws(text),
        [
            Flaw::FieldType { code: code(), field: "when".to_owned(), given: "date".to_owned() },
            Flaw::FieldName { code: code(), field: "type".to_owned() },
            Flaw::FieldName { code: code(), field: "Big".to_owned() },
        ]
    );
}

#[test]
fn refuses_templates_with_undeclared_fields_or_stray_braces() {
    let code = || "a.b.c".to_owned();
    let text = "[field]\npath = \"The file.\"\n[notice.\"a.b.c\"]\nfields = { path = \"path\" }\nmessage = \"{path} {route}\"\nfix = \"oops }\"";
    assert_eq!(
        flaws(text),
        [
            Flaw::Undeclared { code: code(), part: Part::Message, field: "route".to_owned() },
            Flaw::Brace { code: code(), part: Part::Fix, at: 5 },
        ]
    );
    let open = "[notice.\"a.b.c\"]\nmessage = \"x {path\"";
    assert_eq!(flaws(open), [Flaw::Brace { code: code(), part: Part::Message, at: 2 }]);
}

#[test]
fn refuses_a_field_without_a_doc_and_a_doc_without_a_field() {
    let text =
        "[field]\nroute = \"The route.\"\nsize = 3\n[notice.\"a.b.c\"]\nfields = { path = \"path\" }\nmessage = \"x\"";
    assert_eq!(
        flaws(text),
        [
            Flaw::FieldDoc { field: "size".to_owned() },
            Flaw::Undocumented { code: "a.b.c".to_owned(), field: "path".to_owned() },
            Flaw::Unused { field: "route".to_owned() },
        ]
    );
}
