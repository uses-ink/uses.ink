//! Problems and announcements through their coded form, and the codes and fields it refuses.

use model::coded::{Coded, Field, Kind, Refused};
use model::{Announcement, Bytes, ContentPath, Problem};

fn path(text: &str) -> ContentPath {
    text.parse().unwrap()
}

#[test]
fn every_kind_of_field_comes_back_as_it_went() {
    let problems = [
        Problem::EngineSiteRefused,
        Problem::SourceBudgetFile { path: path("a.md"), size: Bytes(30), limit: Bytes(25) },
        Problem::LinkTargetAmbiguous { written: "x".to_owned(), candidates: vec![path("a/x.md"), path("b/x.md")] },
    ];
    for problem in problems {
        assert_eq!(Problem::try_from(Coded::from(&problem)), Ok(problem));
    }
    let notice = Announcement::CodeLanguageUnknown { language: "cobol".to_owned() };
    assert_eq!(Announcement::try_from(Coded::from(&notice)), Ok(notice));
}

#[test]
fn a_code_no_entry_has_is_refused() {
    let refused = Problem::try_from(Coded::new("nothing.like.this", Vec::new()));
    assert_eq!(refused, Err(Refused::Unknown { code: "nothing.like.this".to_owned() }));
}

#[test]
fn too_few_too_many_and_mistyped_fields_are_refused() {
    let code = "html.element.stripped";
    assert_eq!(
        Problem::try_from(Coded::new(code, Vec::new())),
        Err(Refused::Missing { code: code.to_owned(), given: 0 })
    );
    assert_eq!(
        Problem::try_from(Coded::new(code, vec![Field::Count(3)])),
        Err(Refused::Mismatch { code: code.to_owned(), at: 0, found: Kind::Count })
    );
    let twice = vec![Field::Text("a".to_owned()), Field::Text("b".to_owned())];
    assert_eq!(Problem::try_from(Coded::new(code, twice)), Err(Refused::Extra { code: code.to_owned(), wanted: 1 }));
}
