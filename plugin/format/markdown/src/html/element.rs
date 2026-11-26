//! The allowlist of decision 0027: the HTML elements a page keeps, each as the `Document` node
//! it becomes. Every other element is removed.

use std::fmt;
use std::str::FromStr;

/// An element the allowlist keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Element {
    /// `<kbd>`, a key.
    Keyboard,
    /// `<sup>`.
    Superscript,
    /// `<sub>`.
    Subscript,
    /// `<mark>`.
    Highlight,
    /// `<del>` and `<s>`.
    Strike,
    /// `<a href>`.
    Link,
    /// `<img src>`.
    Image,
    /// `<br>`.
    Break,
    /// `<hr>`, only as a block of its own.
    Rule,
    /// `<details>`, only as blocks of its own around markdown.
    Details,
    /// `<summary>`, only first inside `<details>`.
    Summary,
}

/// A tag name the allowlist does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Unlisted;

impl fmt::Display for Unlisted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the element is not on the allowlist of decision 0027")
    }
}

impl std::error::Error for Unlisted {}

impl FromStr for Element {
    type Err = Unlisted;

    /// An element from its lower-cased tag name.
    fn from_str(name: &str) -> Result<Self, Unlisted> {
        Ok(match name {
            "kbd" => Self::Keyboard,
            "sup" => Self::Superscript,
            "sub" => Self::Subscript,
            "mark" => Self::Highlight,
            "del" | "s" => Self::Strike,
            "a" => Self::Link,
            "img" => Self::Image,
            "br" => Self::Break,
            "hr" => Self::Rule,
            "details" => Self::Details,
            "summary" => Self::Summary,
            _ => return Err(Unlisted),
        })
    }
}
