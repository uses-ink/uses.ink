//! `ink:theme-quiet`, the default theme. Deliberately plain: semantic HTML5 written through
//! `unspliced`, one small stylesheet (system fonts, a readable measure, light and dark from the
//! reader's setting), a stylesheet of highlighting classes, and no JavaScript ever. Its real
//! design is the owner's (open.md T1).
//!
//! It implements [`engine::Theme`] directly. That trait is the native stand-in for the
//! `ink:theme` WIT world: when themes become components, this crate speaks the generated WIT
//! types instead and the engine dependency goes.
//!
//! [`document`] holds the Document-to-HTML walk, apart from the page around it, so a second theme
//! can reuse it.

pub mod document;
mod page;
mod style;

use std::cell::RefCell;

use engine::{BaseUrl, File, Html, Record, Said, Theme};
use model::{Diagnostic, MediaType, Page, Route, Site};

use document::Highlighter;
use page::Layout;

/// The highlighting classes, written from giallo's themes by [`code_css`] and checked by a test,
/// so a build without code never loads giallo.
const CODE_CSS: &str = include_str!("../code.css");

/// The highlighting stylesheet as giallo writes it today: what `code.css` must hold.
pub fn code_css() -> Result<String, String> {
    Highlighter::load().map_err(str::to_owned).and_then(Highlighter::css)
}

/// The theme.
#[derive(Debug)]
pub struct Quiet {
    base: BaseUrl,
}

impl Quiet {
    /// The theme for a site served at `base`, for canonical links and `og:url`.
    pub fn new(base: BaseUrl) -> Self {
        Self { base }
    }
}

impl Theme for Quiet {
    fn name(&self) -> &'static str {
        "ink:theme-quiet"
    }

    fn assets(&self) -> Vec<File> {
        let mut files = Vec::new();
        let style = style::quiet::SHEET.text().as_bytes().to_vec();
        if let Ok(route) = page::STYLE.parse::<Route>() {
            files.push(File { route, media: MediaType::Css, bytes: style });
        }
        let code = CODE_CSS.as_bytes().to_vec();
        if let Ok(route) = page::CODE.parse::<Route>() {
            files.push(File { route, media: MediaType::Css, bytes: code });
        }
        files
    }

    fn render<R: Record + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        let said = RefCell::new(Said::default());
        let layout = Layout { page, site, base: &self.base, said: &said };
        let html = Html::from(layout.html());
        let said = said.into_inner();
        said.diagnostics.into_iter().for_each(|diagnostic| record.diagnose(diagnostic));
        said.notices.into_iter().for_each(|notice| record.notice(notice));
        Ok(html)
    }
}
