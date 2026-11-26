//! `ink:theme-quiet`, the default theme. Deliberately plain: semantic HTML5 written through
//! `unspliced`, one small stylesheet (system fonts, a readable measure, light and dark from the
//! reader's setting), a stylesheet of highlighting classes, and no JavaScript ever. Its real
//! design is the owner's (open.md T1).
//!
//! It implements [`engine::Theme`] directly. That trait is the native stand-in for the
//! `ink:theme` WIT world: when themes become components, this crate speaks the generated WIT
//! types instead and the engine dependency goes.
//!
//! `document` writes a page's document as one [`model::Visit`]; `page` writes the page around
//! it.

mod document;
mod page;
mod style;

use engine::{BaseUrl, File, Html, Record, Theme};
use model::{Diagnostic, MediaType, Page, Problem, Route, Site, Subject};

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

    /// A page the theme could not write whole is `render.page.failed`: a bug in the theme.
    fn render<R: Record + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        let mut html = String::new();
        let layout = Layout { page, site, base: &self.base };
        match layout.write(&mut html, record) {
            Ok(()) => Ok(Html::from(html)),
            Err(unwritten) => {
                let problem = Problem::RenderPageFailed { reason: unwritten.to_string() };
                Err(Diagnostic::new(problem).on(Subject::File(page.file.clone())))
            }
        }
    }
}
