//! The checks over every page's markup that no golden reads for you.

use super::{Fixtures, Pageable};
use build::Event;
use model::{Diagnostic, Problem};
use uses_ink_web::view::{Build, Ended as How, Failed, Serving, Settings, Site, Understood};
use uses_ink_web::{Cause, Failure, Frame, Screen};

/// The checks over a page's markup that no golden reads for you.
pub(crate) struct Structure<'a>(pub(crate) &'a str);

impl Structure<'_> {
    pub(crate) fn tags(&self, name: &str) -> Vec<&str> {
        let open = format!("<{name}");
        self.0
            .match_indices(&open)
            .filter(|(at, _)| self.0[at + open.len()..].starts_with([' ', '>', '/']))
            .map(|(at, _)| &self.0[at..=at + self.0[at..].find('>').unwrap()])
            .collect()
    }

    pub(crate) fn forms(&self) -> Vec<&str> {
        let mut forms = Vec::new();
        let mut rest = self.0;
        while let Some(start) = rest.find("<form") {
            let end = rest[start..].find("</form>").unwrap() + start;
            forms.push(&rest[start..end]);
            rest = &rest[end..];
        }
        forms
    }
}

#[test]
fn every_page_has_one_heading_and_every_form_a_label() {
    for (name, page) in Fixtures::pages() {
        let structure = Structure(&page);
        assert_eq!(structure.tags("h1").len(), 1, "{name}: one h1");
        assert!(page.starts_with("<!doctype html><html lang=\"en\">"), "{name}: a document");
        assert!(page.contains("name=\"viewport\""), "{name}: responsive");
        assert!(page.contains("href=\"#main\""), "{name}: a skip link");
        for form in structure.forms() {
            assert!(form.contains("aria-label=\""), "{name}: a form without a name: {form}");
            let controls = Structure(form);
            let visible = ["input", "select", "textarea"]
                .iter()
                .flat_map(|tag| controls.tags(tag))
                .filter(|tag| !tag.contains("type=\"hidden\""))
                .count();
            if visible > 0 {
                assert!(form.contains("<label"), "{name}: a control without a label: {form}");
            }
        }
        // the one first-party script is the passkey one, as this exact tag; any other script is refused
        let bare = page.replace(r#"<script src="/passkey.js"></script>"#, "");
        assert!(!bare.contains("<script"), "{name}: no script but /passkey.js");
        assert!(!page.contains("javascript:"), "{name}: no script url");
    }
}

#[test]
fn a_refresh_is_sent_only_while_a_build_runs() {
    let pages: std::collections::BTreeMap<String, String> = Fixtures::pages().into_iter().collect();
    for (name, page) in &pages {
        let refreshes = page.contains("http-equiv=\"refresh\"");
        assert_eq!(refreshes, matches!(name.as_str(), "site_paused_building" | "stored_running"), "{name}");
    }
}

#[test]
fn text_from_a_source_never_carries_a_control_character() {
    let hostile = "a\u{1b}]52;c;x\u{7}b\u{9b}c";
    let handle = Fixtures::handle();
    let diagnostic = Diagnostic::new(Problem::SourceFileUnreadable {
        path: Fixtures::path("posts/x.md"),
        reason: hostile.to_owned(),
    });
    let mut site = Site::new(handle.clone(), Fixtures::address(), Serving::Nothing);
    site.problems = vec![diagnostic.clone()];
    let source = build::Source::Folder(std::path::PathBuf::from(hostile));
    let understood = Understood::new(handle.clone(), source.clone(), Fixtures::decisions());
    let settings = Settings::new(handle.clone(), Fixtures::address(), source, Serving::Nothing);
    let log = Build::new(
        handle.clone(),
        Fixtures::build(1),
        vec![Event::Diagnosed(diagnostic.clone()), Event::Refused(diagnostic)],
        How::Cut(Failure::Store {
            part: uses_ink_web::Part::Records,
            source: Cause::of(&std::io::Error::other(hostile)),
        }),
    );
    let failed = Failed::new(
        Frame::of(&handle, Screen::Site),
        Failure::Store { part: uses_ink_web::Part::Blobs, source: Cause::of(&std::io::Error::other(hostile)) },
    );
    let pages = [site.page(), understood.page(), settings.page(), log.page(), failed.page()];
    for page in &pages {
        assert!(
            !page.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\t')),
            "a control character reached a page"
        );
    }
    assert!(pages.iter().all(|page| page.contains("\\u{1b}")), "the escape is shown, not dropped");
}
