//! Highlighting as a host capability (decision 0039): a theme asks a [`Highlight`] for a code
//! block's [`Run`]s and styles each [`Capture`] as it likes. The host implements it with
//! tree-sitter grammars; a theme never links a highlighter, and never sees a capture name as
//! text.

use std::fmt;

/// Code split into classed runs, by language.
///
/// ```
/// use std::convert::Infallible;
///
/// use uses_ink_model::highlight::{Capture, Highlight, Run, Unhighlighted};
///
/// /// Knows one language, in which every word is a keyword.
/// struct Shout;
///
/// impl Highlight for Shout {
///     type Error = Infallible;
///
///     fn highlight<'c>(&self, language: &str, code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<Infallible>> {
///         match language {
///             "shout" => Ok(vec![Run { capture: Some(Capture::Keyword), text: code }]),
///             _ => Err(Unhighlighted::Unknown),
///         }
///     }
/// }
///
/// assert!(matches!(Shout.highlight("rust", "fn"), Err(Unhighlighted::Unknown)));
/// ```
pub trait Highlight {
    /// Why code in a language the highlighter knows could not be highlighted.
    type Error: std::error::Error + 'static;

    /// `code` in `language`, as written after a code fence, split into runs that cover all of
    /// it in order.
    fn highlight<'c>(&self, language: &str, code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<Self::Error>>;
}

impl<H: Highlight + ?Sized> Highlight for &H {
    type Error = H::Error;

    fn highlight<'c>(&self, language: &str, code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<Self::Error>> {
        H::highlight(self, language, code)
    }
}

/// One stretch of code with one capture, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run<'c> {
    /// What the grammar says this text is; `None` for text no capture styles.
    pub capture: Option<Capture>,
    /// The text, borrowed from the code.
    pub text: &'c str,
}

/// Why code was not highlighted. The code is still shown, as written.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unhighlighted<E> {
    /// No grammar knows the language.
    Unknown,
    /// The grammar for the language failed: a bug in the host, not in the page.
    Failed(E),
}

impl<E> fmt::Display for Unhighlighted<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Unknown => "no grammar knows the language",
            Self::Failed(_) => "the grammar for the language failed",
        })
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Unhighlighted<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unknown => None,
            Self::Failed(source) => Some(source),
        }
    }
}

/// What a grammar says a stretch of code is: the closed set every theme styles. Tree-sitter's
/// capture names (`keyword.return`, `function.builtin`, `markup.heading.2`, and the older
/// nvim-treesitter names such as `include` or `repeat`) map onto these by their family; the
/// host does the mapping, so a theme never reads a capture name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Capture {
    /// `attribute`: `#[derive]`, `@decorator`.
    Attribute,
    /// `comment` and its kinds.
    Comment,
    /// `constant`, `boolean`: `true`, `None`, `MAX`.
    Constant,
    /// `constructor`: an enum variant or a class made.
    Constructor,
    /// `diff.minus`: a removed line.
    Deleted,
    /// `embedded`: code of another language inside this one.
    Embedded,
    /// `markup.italic`.
    Emphasis,
    /// `error`: what the grammar could not parse.
    Error,
    /// `function`, `method`, and their calls and builtins.
    Function,
    /// `markup.heading`.
    Heading,
    /// `diff.plus`: an added line.
    Inserted,
    /// `keyword` and every kind of it.
    Keyword,
    /// `label`: a loop label, a goto target.
    Label,
    /// `markup.link`.
    Link,
    /// `markup.raw`: code inside prose.
    Literal,
    /// `function.macro`, `preproc.macro`.
    Macro,
    /// `module`, `namespace`.
    Module,
    /// `number`, `float`.
    Number,
    /// `operator`.
    Operator,
    /// `property`, a field read or written.
    Property,
    /// `punctuation`, brackets and delimiters, and list markers in markup.
    Punctuation,
    /// `markup.strikethrough`.
    Strike,
    /// `string`, `character`, escapes and regexes.
    String,
    /// `markup.bold`.
    Strong,
    /// `tag`: an HTML or XML element name.
    Tag,
    /// `type` and its builtins and parameters.
    Type,
    /// `variable`, `parameter`.
    Variable,
}

impl Capture {
    /// Every capture, in the order above.
    pub const ALL: &'static [Self] = &[
        Self::Attribute,
        Self::Comment,
        Self::Constant,
        Self::Constructor,
        Self::Deleted,
        Self::Embedded,
        Self::Emphasis,
        Self::Error,
        Self::Function,
        Self::Heading,
        Self::Inserted,
        Self::Keyword,
        Self::Label,
        Self::Link,
        Self::Literal,
        Self::Macro,
        Self::Module,
        Self::Number,
        Self::Operator,
        Self::Property,
        Self::Punctuation,
        Self::Strike,
        Self::String,
        Self::Strong,
        Self::Tag,
        Self::Type,
        Self::Variable,
    ];
}
