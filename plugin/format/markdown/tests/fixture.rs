//! Every `fixture/<name>.md` read and compared with `fixture/<name>.snap`: the [`model::Read`] as
//! pretty JSON. A changed or missing golden is written beside it as `.snap.new` and the run
//! fails; `cargo insta review` (or `INSTA_UPDATE=always`) accepts it once read.

use std::path::Path;

use model::ContentPath;

#[test]
fn fixtures_read_as_expected() {
    insta::glob!("../fixture", "*.md", |input| {
        let name = input.file_stem().unwrap().to_str().unwrap();
        let bytes = std::fs::read(input).unwrap();
        let read = uses_ink_format_markdown::read(&ContentPath::parse(format!("{name}.md")).unwrap(), &bytes);
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path("../fixture");
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        settings.remove_snapshot_suffix();
        settings.set_input_file(input);
        settings.bind(|| insta::assert_snapshot!(name, serde_json::to_string_pretty(&read).unwrap()));
    });
}

/// A golden whose input is gone fails, so no stale golden stays behind.
#[test]
fn every_golden_has_an_input() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture");
    let orphans: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext != "md"))
        .filter(|path| !path.with_extension("md").exists() || path.extension().is_some_and(|ext| ext != "snap"))
        .collect();
    assert!(orphans.is_empty(), "files in fixture/ that are not an input with its golden: {orphans:?}");
}
