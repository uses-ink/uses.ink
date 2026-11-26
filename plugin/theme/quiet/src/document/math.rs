//! TeX to `MathML` through `math-core`, behind our types: nothing else in the crate names it.

use std::sync::OnceLock;

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};

static CONVERTER: OnceLock<Result<LatexToMathML, String>> = OnceLock::new();

/// Whether math sits in a line of text or on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Display {
    Inline,
    Block,
}

/// The converter, made on first use.
#[derive(Clone, Copy)]
pub(crate) struct Math(&'static LatexToMathML);

impl std::fmt::Debug for Math {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Math")
    }
}

impl Math {
    pub(crate) fn load() -> Result<Self, String> {
        CONVERTER
            .get_or_init(|| {
                LatexToMathML::new(MathCoreConfig::default(), vec![]).map_err(|(error, _, _)| error.to_string())
            })
            .as_ref()
            .map(Self)
            .map_err(Clone::clone)
    }

    /// `tex` as a `<math>` element, escaped by math-core, or why it does not parse.
    pub(crate) fn mathml(self, tex: &str, display: Display) -> Result<String, String> {
        let converter = self.0;
        let display = match display {
            Display::Inline => MathDisplay::Inline,
            Display::Block => MathDisplay::Block,
        };
        converter.convert_with_local_state(tex, display).map(|result| result.mathml).map_err(|error| error.to_string())
    }
}
