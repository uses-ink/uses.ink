//! A stand-in theme that writes a few tags.

use engine::{File, Html, Sink, Theme};
use model::{Announcement, Block, Diagnostic, ImageSource, Inline, LinkTarget, Notice, Page, Problem, Site};
use std::fmt::Write as _;

/// A stand-in theme: the title, then each paragraph's link or image as where it points.
#[derive(Debug, Default)]
pub struct Plain {
    pub files: Vec<File>,
    /// Pages it refuses, by title, each after a notice that it was asked to.
    pub refuses: Vec<String>,
}

impl Theme for Plain {
    fn name(&self) -> &'static str {
        "plain"
    }

    fn assets(&self) -> Vec<File> {
        self.files.clone()
    }

    fn render<R: Sink + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        if self.refuses.contains(&page.title) {
            record.notice(Notice::new(Announcement::CodeLanguageUnknown { language: "refused".to_owned() }));
            return Err(Diagnostic::new(Problem::HtmlElementStripped { element: "refused".to_owned() }));
        }
        let mut html = format!("<!doctype html><title>{}</title>", page.title);
        for block in &page.body.blocks {
            if let Block::Paragraph(inlines) = block {
                for inline in inlines {
                    let _infallible = write!(html, "<p>{}</p>", shown(inline, site));
                }
            }
        }
        Ok(Html::from(html))
    }
}

fn shown(inline: &Inline, site: &Site) -> String {
    match inline {
        Inline::Link { target: LinkTarget::Page { id, anchor }, .. } => {
            let route = site.page(*id).map(|p| p.route.to_string()).unwrap();
            anchor.as_ref().map_or(route.clone(), |a| format!("{route}#{a}"))
        }
        Inline::Link { target: LinkTarget::Anchor(a), .. } => format!("#{a}"),
        Inline::Link { target: LinkTarget::External(url), .. } => url.to_string(),
        Inline::Link { target: LinkTarget::Unresolved { written }, .. } => format!("text:{written}"),
        Inline::Image { source: ImageSource::Asset(path), .. } => {
            site.assets().iter().find(|a| &a.file == path).map(|a| a.route.to_string()).unwrap()
        }
        Inline::Image { source, .. } => format!("{source:?}"),
        Inline::Text(text) => text.clone(),
        other => format!("{other:?}"),
    }
}
