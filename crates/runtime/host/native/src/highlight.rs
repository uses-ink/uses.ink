//! Code highlighted by arborium's tree-sitter grammars, behind model's
//! [`Highlight`](model::highlight::Highlight): nothing else in the crate names arborium.
//!
//! Only the grammars enabled as features of the workspace's `arborium` are built in. A grammar
//! is compiled the first time a block of its language is highlighted, then kept for the
//! process: every [`Grammars`] shares one store.

use std::fmt;
use std::sync::{Arc, LazyLock};

use arborium::advanced::Span;
use arborium::{GrammarStore, Highlighter};
use arborium_theme::highlights::{ThemeSlot, capture_to_slot};
use model::highlight::{Capture, Highlight, Run, Unhighlighted};

/// The grammars compiled so far in this process.
static STORE: LazyLock<Arc<GrammarStore>> = LazyLock::new(Arc::default);

/// The built-in grammars.
#[derive(Clone)]
pub struct Grammars {
    store: Arc<GrammarStore>,
}

impl Default for Grammars {
    /// The process's grammars.
    fn default() -> Self {
        Self { store: Arc::clone(&STORE) }
    }
}

impl fmt::Debug for Grammars {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Grammars")
    }
}

impl Highlight for Grammars {
    type Error = GrammarError;

    /// Names are matched without case, and arborium's aliases (`rs`, `js`, `sh`, `yml`) hold.
    fn highlight<'c>(&self, language: &str, code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<GrammarError>> {
        let language = language.to_ascii_lowercase();
        let mut highlighter = Highlighter::with_store(Arc::clone(&self.store));
        let spans = highlighter.highlight_spans(&language, code).map_err(|error| match error {
            arborium::Error::UnsupportedLanguage { .. } => Unhighlighted::Unknown,
            source => Unhighlighted::Failed(GrammarError::Parse { source }),
        })?;
        Layers::from(spans).runs(code).map_err(Unhighlighted::Failed)
    }
}

/// Why a grammar did not highlight code in its language.
#[derive(Debug)]
#[non_exhaustive]
pub enum GrammarError {
    /// arborium could not parse the code or run the grammar's queries.
    Parse {
        /// arborium's error.
        source: arborium::Error,
    },
    /// A span starts or ends inside a character of the code.
    Boundary {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for GrammarError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { .. } => out.write_str("the grammar could not parse the code"),
            Self::Boundary { at } => write!(out, "the grammar cut the code inside a character, at byte {at}"),
        }
    }
}

impl std::error::Error for GrammarError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse { source } => Some(source),
            Self::Boundary { .. } => None,
        }
    }
}

/// The styled spans of one block as the grammar captured them: nested, crossing, and sometimes
/// several on one range.
#[derive(Debug)]
struct Layers(Vec<Layer>);

/// One styled span.
#[derive(Clone, Copy, Debug)]
struct Layer {
    start: usize,
    end: usize,
    capture: Capture,
    /// The query pattern that captured it: of two on one range, the later pattern wins, as in
    /// tree-sitter.
    pattern: u32,
}

/// Where a layer starts or ends. A layer ending at a position closes before one starting there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Edge {
    Close,
    Open,
}

impl From<Vec<Span>> for Layers {
    /// Keeps the spans a capture styles and that hold text: `spell`, `none` and the like style
    /// nothing, and a grammar run again on an injection can capture an empty span.
    fn from(spans: Vec<Span>) -> Self {
        let layers = spans.into_iter().filter(|span| span.start < span.end).filter_map(|span| {
            let capture = Slot(capture_to_slot(&span.capture)).capture()?;
            Some(Layer { start: span.start as usize, end: span.end as usize, capture, pattern: span.pattern_index })
        });
        Self(layers.collect())
    }
}

impl Layers {
    /// The runs covering all of `code`, each byte under the innermost layer over it, and
    /// neighbours with one capture joined.
    fn runs(mut self, code: &str) -> Result<Vec<Run<'_>>, GrammarError> {
        // Outer layers first, so the one on top of the stack is the innermost.
        self.0.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)).then(a.pattern.cmp(&b.pattern)));
        let mut edges: Vec<(usize, Edge, usize)> = Vec::with_capacity(self.0.len() * 2);
        for (index, layer) in self.0.iter().enumerate() {
            edges.push((layer.start, Edge::Open, index));
            edges.push((layer.end, Edge::Close, index));
        }
        edges.sort_unstable();
        let mut runs = Runs { code, written: Vec::new(), at: 0 };
        let mut open: Vec<usize> = Vec::new();
        for (position, edge, index) in edges {
            let capture = open.last().and_then(|&top| self.0.get(top)).map(|layer| layer.capture);
            runs.push(position.min(code.len()), capture)?;
            match edge {
                Edge::Open => open.push(index),
                Edge::Close => {
                    if let Some(at) = open.iter().rposition(|&layer| layer == index) {
                        open.remove(at);
                    }
                }
            }
        }
        runs.push(code.len(), None)?;
        Ok(runs.written)
    }
}

/// Runs written so far, up to byte `at` of `code`.
#[derive(Debug)]
struct Runs<'c> {
    code: &'c str,
    written: Vec<Run<'c>>,
    at: usize,
}

impl Runs<'_> {
    /// The code from where the last run ended up to `end`, under `capture`.
    fn push(&mut self, end: usize, capture: Option<Capture>) -> Result<(), GrammarError> {
        if end <= self.at {
            return Ok(());
        }
        let text = self.code.get(self.at..end).ok_or(GrammarError::Boundary { at: self.at })?;
        self.at = end;
        match self.written.last_mut() {
            Some(last) if last.capture == capture => {
                let start = end - last.text.len() - text.len();
                last.text = self.code.get(start..end).ok_or(GrammarError::Boundary { at: start })?;
            }
            _ => self.written.push(Run { capture, text }),
        }
        Ok(())
    }
}

/// arborium's theme slot, which its table maps every capture name to.
#[derive(Clone, Copy, Debug)]
struct Slot(ThemeSlot);

impl Slot {
    /// The capture it is; `None` for the slot that styles nothing.
    fn capture(self) -> Option<Capture> {
        Some(match self.0 {
            ThemeSlot::Keyword => Capture::Keyword,
            ThemeSlot::Function => Capture::Function,
            ThemeSlot::String => Capture::String,
            ThemeSlot::Comment => Capture::Comment,
            ThemeSlot::Type => Capture::Type,
            ThemeSlot::Variable => Capture::Variable,
            ThemeSlot::Constant => Capture::Constant,
            ThemeSlot::Number => Capture::Number,
            ThemeSlot::Operator => Capture::Operator,
            ThemeSlot::Punctuation => Capture::Punctuation,
            ThemeSlot::Property => Capture::Property,
            ThemeSlot::Attribute => Capture::Attribute,
            ThemeSlot::Tag => Capture::Tag,
            ThemeSlot::Macro => Capture::Macro,
            ThemeSlot::Label => Capture::Label,
            ThemeSlot::Namespace => Capture::Module,
            ThemeSlot::Constructor => Capture::Constructor,
            ThemeSlot::Title => Capture::Heading,
            ThemeSlot::Strong => Capture::Strong,
            ThemeSlot::Emphasis => Capture::Emphasis,
            ThemeSlot::Link => Capture::Link,
            ThemeSlot::Literal => Capture::Literal,
            ThemeSlot::Strikethrough => Capture::Strike,
            ThemeSlot::DiffAdd => Capture::Inserted,
            ThemeSlot::DiffDelete => Capture::Deleted,
            ThemeSlot::Embedded => Capture::Embedded,
            ThemeSlot::Error => Capture::Error,
            ThemeSlot::None => return None,
        })
    }
}
