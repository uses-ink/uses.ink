//! Every `fixture/<name>.md` read and compared with `fixture/<name>.snap`: the [`model::Read`] as
//! pretty JSON, through the shared harness.

use model::ContentPath;

#[test]
fn fixtures_read_as_expected() {
    fixture::files("fixture", "md", |path, bytes| {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();
        let read = uses_ink_format_markdown::read(&ContentPath::parse(name).unwrap(), bytes);
        serde_json::to_string_pretty(&read).unwrap()
    });
}
