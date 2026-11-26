//! Code blocks highlighted by `giallo`, behind our types: nothing else in the crate names it.
//!
//! Spans carry classes, never inline styles (the reader CSP allows only the site's stylesheets),
//! and [`Highlighter::css`] writes the classes of a light and a dark theme into one file, the dark
//! one under `prefers-color-scheme: dark`. giallo's built-in grammars and themes load once per
//! process, the first time a page has code in a language.

use std::sync::OnceLock;

use engine::Record;
use giallo::{HighlightOptions, HtmlRenderer, Registry, RenderOptions, ThemeVariant};
use model::{Announcement, Diagnostic, Notice, Problem};
use unspliced::{Raw, html};

use super::Writer;

/// The prefix of every class giallo writes.
const PREFIX: &str = "hl-";
const LIGHT: &str = "github-light";
const DARK: &str = "github-dark";

/// The registry, or why it did not load.
static REGISTRY: OnceLock<Result<Registry, String>> = OnceLock::new();

/// giallo's grammars and themes, loaded.
#[derive(Clone, Copy)]
pub(crate) struct Highlighter(&'static Registry);

impl std::fmt::Debug for Highlighter {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Highlighter")
    }
}

/// Code as giallo highlighted it, or why it could not.
#[derive(Debug)]
pub(crate) enum Highlighted {
    /// `<pre><code>` with classed spans, escaped by giallo.
    Html(String),
    /// No grammar knows the language.
    Unknown,
    /// The built-in registry did not load: a bug in the build, not in the page.
    Broken(String),
}

impl Highlighter {
    /// The built-in registry, loaded on first use.
    pub(crate) fn load() -> Result<Self, &'static str> {
        let loaded = REGISTRY.get_or_init(|| {
            let mut registry = Registry::builtin().map_err(|error| error.to_string())?;
            registry.link_grammars();
            Ok(registry)
        });
        loaded.as_ref().map(Self).map_err(String::as_str)
    }

    /// `text` in `language`, highlighted.
    pub(crate) fn highlight(self, language: &str, text: &str) -> Highlighted {
        let registry = self.0;
        let language = language.to_ascii_lowercase();
        if !registry.contains_grammar(&language) {
            return Highlighted::Unknown;
        }
        let options = HighlightOptions::new(&language, ThemeVariant::Dual { light: LIGHT, dark: DARK });
        match registry.highlight(text, &options) {
            Ok(highlighted) => {
                let renderer = HtmlRenderer {
                    css_class_prefix: Some(PREFIX.to_owned()),
                    add_color_scheme: false,
                    ..HtmlRenderer::default()
                };
                Highlighted::Html(renderer.render(&highlighted, &RenderOptions::default()))
            }
            Err(error) => Highlighted::Broken(error.to_string()),
        }
    }

    /// The classes of the light theme, then the dark theme's for a dark reader.
    pub(crate) fn css(self) -> Result<String, String> {
        let (light, dark) = self.0.generate_dual_css(LIGHT, DARK, PREFIX).map_err(|error| error.to_string())?;
        Ok(format!("{light}\n@media (prefers-color-scheme: dark) {{\n{dark}}}\n"))
    }
}

impl<R: Record + ?Sized> Writer<'_, R> {
    /// A code block, highlighted when a grammar knows its language.
    pub(super) fn code(&mut self, language: Option<&str>, text: &str) {
        let plain = html! { pre { code { (text) } } };
        let Some(language) = language else { return self.put(plain) };
        let highlighted = match Highlighter::load() {
            Ok(highlighter) => highlighter.highlight(language, text),
            Err(reason) => Highlighted::Broken(reason.to_owned()),
        };
        match highlighted {
            // giallo escapes the code's text and writes only spans with its own classes.
            Highlighted::Html(html) => self.put(Raw::trusted(&html)),
            Highlighted::Unknown => {
                let notice = Announcement::CodeLanguageUnknown { language: language.to_owned() };
                let subject = self.subject();
                self.record.notice(Notice::new(notice).on(subject));
                self.put(plain);
            }
            Highlighted::Broken(reason) => {
                let problem = Problem::RenderPageFailed { reason };
                let subject = self.subject();
                self.record.diagnose(Diagnostic::new(problem).on(subject));
                self.put(plain);
            }
        }
    }
}
