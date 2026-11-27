//! Every `fixture/publishing/<name>.md` selected and cleared: the text each edit gives, or why it
//! is refused, compared with `<name>.snap`, through the shared harness.

use std::fmt::Write as _;

use uses_ink_format_markdown::{Edit, Publishing, Refusal};

/// What one edit did, as text.
fn shown(edit: &Result<Edit, Refusal>) -> String {
    match edit {
        Ok(Edit { was, text: Some(text), .. }) => format!("was {was:?}, now:\n{text:?}\n"),
        Ok(Edit { was, text: None, .. }) => format!("was {was:?}, unchanged\n"),
        Err(refusal) => format!("refused: {refusal} ({refusal:?})\n"),
    }
}

#[test]
fn files_are_selected_and_cleared_as_the_snapshots_say() {
    fixture::files("fixture/publishing", "md", |_, bytes| {
        let mut out = String::new();
        match Publishing::of(bytes) {
            Ok(publishing) => {
                let _infallible =
                    write!(out, "select: {}clear: {}", shown(&publishing.select()), shown(&publishing.clear()));
            }
            Err(refusal) => {
                let _infallible = writeln!(out, "refused: {refusal} ({refusal:?})");
            }
        }
        out
    });
}

#[test]
fn a_selected_file_reads_as_published_and_a_cleared_one_does_not() {
    let text = "---\ntitle: Hello\n---\nBody.\n";
    let selected = Publishing::of(text.as_bytes()).unwrap().select().unwrap().text.unwrap();
    let again = Publishing::of(selected.as_bytes()).unwrap();
    assert_eq!(again.select().unwrap().text, None, "selecting twice changes nothing");
    let cleared = again.clear().unwrap().text.unwrap();
    assert_eq!(cleared, text, "clearing restores the file byte for byte");
}
