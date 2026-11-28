//! The allowlist of decision 0027: the HTML elements a page keeps, and what each becomes. Every
//! other element is removed.

use std::fmt;
use std::str::FromStr;

use model::Inline;

use super::inline::{Plain, Written};
use crate::html::Tag;

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

/// What an allowed opening tag does among inlines.
#[derive(Debug)]
pub(crate) enum Opening {
    /// It is a node of its own: `<br>`, `<img>`.
    Leaf(Inline),
    /// It wraps what follows, up to its closing tag.
    Wrap(Wrap),
}

/// The node an element wrapping inlines becomes once closed.
#[derive(Debug)]
pub(crate) enum Wrap {
    /// `<kbd>`: a key, its text only.
    Keyboard,
    /// `<sup>`.
    Superscript,
    /// `<sub>`.
    Subscript,
    /// `<mark>`.
    Highlight,
    /// `<del>`, `<s>`.
    Strike,
    /// `<a href>`, pointing where `href` does.
    Link(model::LinkTarget),
}

impl Element {
    /// Each tag name the allowlist holds, lower case, and its element.
    const NAMES: &'static [(&'static str, Self)] = &[
        ("kbd", Self::Keyboard),
        ("sup", Self::Superscript),
        ("sub", Self::Subscript),
        ("mark", Self::Highlight),
        ("del", Self::Strike),
        ("s", Self::Strike),
        ("a", Self::Link),
        ("img", Self::Image),
        ("br", Self::Break),
        ("hr", Self::Rule),
        ("details", Self::Details),
        ("summary", Self::Summary),
    ];

    /// What its opening tag `tag` does among inlines, or `None` when it is refused there: a
    /// block element, or a link or image missing the attribute it needs. Only `href`, `src` and
    /// `alt` are read; no other attribute is ever written out, `on*` handlers included.
    pub(crate) fn inline(self, tag: &Tag<'_>) -> Option<Opening> {
        Some(Opening::Wrap(match self {
            Self::Keyboard => Wrap::Keyboard,
            Self::Superscript => Wrap::Superscript,
            Self::Subscript => Wrap::Subscript,
            Self::Highlight => Wrap::Highlight,
            Self::Strike => Wrap::Strike,
            Self::Link => Wrap::Link(Written(&tag.attribute("href")?).into()),
            Self::Break => return Some(Opening::Leaf(Inline::Break)),
            Self::Image => {
                let source = Written(&tag.attribute("src")?).into();
                // not a fallback: no `alt` is no alt text, as `![](src)` has none
                let alt = tag.attribute("alt").unwrap_or_default();
                return Some(Opening::Leaf(Inline::Image { source, alt }));
            }
            Self::Rule | Self::Details | Self::Summary => return None,
        }))
    }
}

impl Wrap {
    /// The node around `children`.
    pub(crate) fn around(self, children: Vec<Inline>) -> Inline {
        match self {
            Self::Keyboard => Inline::Keyboard(Plain::of(&children)),
            Self::Superscript => Inline::Superscript(children),
            Self::Subscript => Inline::Subscript(children),
            Self::Highlight => Inline::Highlight(children),
            Self::Strike => Inline::Strike(children),
            Self::Link(target) => Inline::Link { target, children },
        }
    }
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
        Self::NAMES.iter().find(|(listed, _)| *listed == name).map(|&(_, element)| element).ok_or(Unlisted)
    }
}
