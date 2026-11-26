//! What a decision can decide: [`Value`], implemented once per value type, and generically for
//! `Option<T>` and `Vec<T>`. A surface a value appears on is a trait of its own, so a value type
//! opts into it: [`FromFront`] for frontmatter here, `Toml` for `ink.toml` in the config crate.

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Author, ContentPath, Language, Layout, PageKind, Root, Slug, Tag, When};

mod front;

pub use front::{Cause, Expected, Form, FromFront, Raw, Refusal, Wanted};

/// A value a decision decides: compared against alternatives, stored, and shown to a person.
pub trait Value: Clone + Eq + fmt::Debug + Serialize + DeserializeOwned {
    /// Writes the value as `ink explain` and notices show it: `a post`, `“Hello”`, `none`.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result;

    /// Whether it holds nothing: none, or an empty list. A code words it as its own `NONE`
    /// ([`Word`](crate::code::Word)).
    fn is_empty(&self) -> bool {
        false
    }

    /// The value as `Display`, through [`Value::show`].
    fn shown(&self) -> Shown<'_, Self> {
        Shown(self)
    }
}

/// A value shown through [`Value::show`]. `Option` and `Vec` cannot implement `Display`
/// themselves, so every value is shown through this.
#[derive(Clone, Copy, Debug)]
pub struct Shown<'a, V: ?Sized>(pub &'a V);

impl<V: Value> fmt::Display for Shown<'_, V> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.show(out)
    }
}

impl<T: Value> Value for Option<T> {
    /// The value, or `none`.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Some(value) => value.show(out),
            None => out.write_str("none"),
        }
    }

    fn is_empty(&self) -> bool {
        self.is_none()
    }
}

impl<T: Value> Value for Vec<T> {
    /// The values joined by `, `, or `none`.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((first, rest)) = self.split_first() else { return out.write_str("none") };
        first.show(out)?;
        rest.iter().try_for_each(|value| {
            out.write_str(", ")?;
            value.show(out)
        })
    }

    fn is_empty(&self) -> bool {
        <[T]>::is_empty(self)
    }
}

impl Value for String {
    /// In quotes: `“Hello”`.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "“{self}”")
    }
}

impl Value for bool {
    /// `yes` or `no`.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(if *self { "yes" } else { "no" })
    }
}

impl Value for Root {
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Top => out.write_str("the top of the snapshot"),
            Self::Folder(folder) => write!(out, "`{folder}/`"),
        }
    }
}

impl Value for Layout {
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Blog => "a blog",
            Self::Vault => "a vault",
            Self::Docs => "docs",
            Self::Flat => "flat",
        })
    }
}

impl Value for PageKind {
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Home => "the home page",
            Self::Post => "a post",
            Self::Page => "a page",
        })
    }
}

impl Value for Tag {
    /// As the author wrote it.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.written)
    }
}

impl Value for When {
    /// `2024-03-01`, or the instant in UTC.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, out)
    }
}

impl Value for Author {
    /// The name.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, out)
    }
}

impl Value for Slug {
    /// In backticks: `` `hello` ``.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{self}`")
    }
}

impl Value for Language {
    /// In backticks: `` `fr-CH` ``.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{self}`")
    }
}

impl Value for ContentPath {
    /// In backticks: `` `posts/hello.md` ``.
    fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{self}`")
    }
}
