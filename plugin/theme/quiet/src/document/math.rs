//! TeX as `MathML` through `math-core`, behind our types: nothing else in the crate names it.
//! Its output is read back through an allowlist ([`allow`]) before a page holds it.

mod allow;

use std::fmt;
use std::sync::OnceLock;

use engine::Record;
use math_core::{LatexError, LatexToMathML, MathCoreConfig, MathDisplay};
use model::highlight::Highlight;
use model::{Diagnostic, Problem};
use unspliced::{Raw, html};

use self::allow::{Mathml, Refused};
use super::Writer;
use crate::style::quiet;

/// The converter, or why it could not be made.
static CONVERTER: OnceLock<Result<LatexToMathML, Box<LatexError>>> = OnceLock::new();

/// Whether math sits in a line of text or on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Placement {
    Inline,
    Block,
}

/// The converter, made on first use.
#[derive(Clone, Copy)]
struct Math(&'static LatexToMathML);

impl fmt::Debug for Math {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Math")
    }
}

/// Why TeX was not converted. Its words are math-core's own.
#[derive(Debug)]
enum Unparsed {
    /// The converter could not be made: a bug in the theme, not in the page.
    Converter(&'static LatexError),
    /// The TeX does not parse.
    Syntax(Box<LatexError>),
    /// The converter wrote markup the allowlist does not hold.
    Refused(Refused),
}

impl fmt::Display for Unparsed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Converter(error) => fmt::Display::fmt(error, out),
            Self::Syntax(error) => fmt::Display::fmt(error, out),
            Self::Refused(refused) => fmt::Display::fmt(refused, out),
        }
    }
}

impl Math {
    fn load() -> Result<Self, Unparsed> {
        let made = CONVERTER
            .get_or_init(|| LatexToMathML::new(MathCoreConfig::default(), vec![]).map_err(|(error, _, _)| error));
        made.as_ref().map(Self).map_err(|error| Unparsed::Converter(error))
    }

    /// `tex` as a `<math>` element, escaped by math-core and read through the allowlist, or why
    /// it does not parse.
    fn mathml(self, tex: &str, placement: Placement) -> Result<Mathml, Unparsed> {
        let display = match placement {
            Placement::Inline => MathDisplay::Inline,
            Placement::Block => MathDisplay::Block,
        };
        let converted = self.0.convert_with_local_state(tex, display).map_err(Unparsed::Syntax)?;
        Mathml::try_from(converted.mathml.as_str()).map_err(Unparsed::Refused)
    }
}

impl<H: Highlight, R: Record + ?Sized> Writer<'_, H, R> {
    /// Math as `MathML`, or as written with `math.syntax.invalid` when it does not parse.
    pub(super) fn math(&mut self, tex: &str, placement: Placement) {
        match Math::load().and_then(|math| math.mathml(tex, placement)) {
            // math-core escapes every character it writes, and the allowlist read it back.
            Ok(mathml) => self.put(Raw::trusted(mathml.as_ref())),
            Err(unparsed) => {
                let problem = Problem::MathSyntaxInvalid { tex: tex.to_owned(), reason: unparsed.to_string() };
                let subject = self.subject();
                self.record.diagnose(Diagnostic::new(problem).on(subject));
                self.put(html! { code.quiet::UNPARSED { (tex) } });
            }
        }
    }
}
