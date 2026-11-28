//! [`Toml`]: how a decided value is spelled in `ink.toml`, implemented once per value type and
//! generically for `Option<T>` and `Vec<T>`, so a new code needs no reader or writer of its
//! own. Reading goes through [`Node`] and writing through [`Written`], so this module sees
//! neither `toml` nor `toml_edit`.

mod shape;

use model::record::PluginId;
use model::{Author, ContentPath, Language, Layout, PageKind, Root, Slug, Tag, Value, When};

pub(crate) use shape::{Cause, Expected, Shape};

use crate::read::{Node, Refused};
use crate::write::{Unwritable, Written};

/// A value an `ink.toml` pin can hold. A value [`Toml::write`] gives, [`Toml::read`] reads
/// back as the same value.
pub(crate) trait Toml: Value {
    /// What it accepts, for `config.value.invalid`.
    const SHAPE: Shape<'static>;

    /// The value `node` holds, or every part of it that is wrong.
    fn read(node: Node<'_>) -> Result<Self, Refused>;

    /// The value as TOML.
    fn write(&self) -> Result<Written, Unwritable>;
}

impl<T: Toml> Toml for Option<T> {
    /// `false` is the value type's none; [`Shape::pin`] words it as the pinned code does.
    const SHAPE: Shape<'static> = Shape::Optional { shape: &T::SHAPE, none: "none" };

    /// `false` is none, as in frontmatter (`date = false` is undated); else a value of `T`. An
    /// `Option<bool>` therefore cannot hold `Some(false)`, and no code decides one.
    fn read(node: Node<'_>) -> Result<Self, Refused> {
        if node.is_false() { Ok(None) } else { T::read(node).map(Some) }
    }

    fn write(&self) -> Result<Written, Unwritable> {
        match self {
            Some(value) => value.write(),
            None => Ok(false.into()),
        }
    }
}

impl<T: Toml> Toml for Vec<T> {
    const SHAPE: Shape<'static> = Shape::Array(&T::SHAPE);

    /// An array, each element a `T`; refused with the span of every element that is not.
    fn read(node: Node<'_>) -> Result<Self, Refused> {
        Refused::gather(node.items()?.map(T::read))
    }

    fn write(&self) -> Result<Written, Unwritable> {
        self.iter().map(T::write).collect()
    }
}

impl Toml for String {
    const SHAPE: Shape<'static> = Shape::Text;

    fn read(node: Node<'_>) -> Result<Self, Refused> {
        node.text().map(str::to_owned)
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok(self.as_str().into())
    }
}

impl Toml for bool {
    const SHAPE: Shape<'static> = Shape::Flag;

    fn read(node: Node<'_>) -> Result<Self, Refused> {
        node.flag()
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok((*self).into())
    }
}

impl Toml for When {
    const SHAPE: Shape<'static> = Shape::When;

    /// A local date is a day; a date and time with an offset is an instant.
    fn read(node: Node<'_>) -> Result<Self, Refused> {
        node.when()
    }

    /// A day as a local date, an instant as a date and time in UTC.
    fn write(&self) -> Result<Written, Unwritable> {
        Written::try_from(*self)
    }
}

impl Toml for Slug {
    const SHAPE: Shape<'static> = Shape::Slug;

    /// Text that already is a slug: `on-shelves`, not `On Shelves`.
    fn read(node: Node<'_>) -> Result<Self, Refused> {
        Self::parse(node.text()?).map_err(|_| node.refused())
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok(self.as_str().into())
    }
}

impl Toml for Language {
    const SHAPE: Shape<'static> = Shape::Language;

    fn read(node: Node<'_>) -> Result<Self, Refused> {
        node.text()?.parse().map_err(|_| node.refused())
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok(self.as_ref().into())
    }
}

impl Toml for Author {
    const SHAPE: Shape<'static> = Shape::Author;

    fn read(node: Node<'_>) -> Result<Self, Refused> {
        Self::try_from(node.text()?.to_owned()).map_err(|_| node.refused())
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok(self.as_ref().into())
    }
}

impl Toml for Tag {
    const SHAPE: Shape<'static> = Shape::Tag;

    /// Written as the author spelled it; the slug is derived again on reading.
    fn read(node: Node<'_>) -> Result<Self, Refused> {
        Self::new(node.text()?).map_err(|_| node.refused())
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok(self.written.as_str().into())
    }
}

impl Toml for ContentPath {
    const SHAPE: Shape<'static> = Shape::Path;

    fn read(node: Node<'_>) -> Result<Self, Refused> {
        Self::parse(node.text()?).map_err(|_| node.refused())
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok(self.as_str().into())
    }
}

impl Toml for Root {
    const SHAPE: Shape<'static> = Shape::Root;

    /// `""` is the top of the source; anything else is a folder's path.
    fn read(node: Node<'_>) -> Result<Self, Refused> {
        match node.text()? {
            "" => Ok(Self::Top),
            _ => ContentPath::read(node).map(Self::Folder),
        }
    }

    fn write(&self) -> Result<Written, Unwritable> {
        match self {
            Self::Top => Ok("".into()),
            Self::Folder(path) => path.write(),
            _ => Err(Unwritable::Unknown),
        }
    }
}

impl Toml for PluginId {
    const SHAPE: Shape<'static> = Shape::Format;

    /// A plugin id that names a format: `ink:format-obsidian`.
    fn read(node: Node<'_>) -> Result<Self, Refused> {
        let id: Self = node.text()?.parse().map_err(|_| node.refused())?;
        if id.is_format() { Ok(id) } else { Err(node.refused()) }
    }

    fn write(&self) -> Result<Written, Unwritable> {
        Ok(self.to_string().as_str().into())
    }
}

impl Toml for Layout {
    const SHAPE: Shape<'static> = Shape::One(&LAYOUTS.names);

    fn read(node: Node<'_>) -> Result<Self, Refused> {
        LAYOUTS.read(node)
    }

    fn write(&self) -> Result<Written, Unwritable> {
        LAYOUTS.write(*self)
    }
}

impl Toml for PageKind {
    const SHAPE: Shape<'static> = Shape::One(&KINDS.names);

    fn read(node: Node<'_>) -> Result<Self, Refused> {
        KINDS.read(node)
    }

    fn write(&self) -> Result<Written, Unwritable> {
        KINDS.write(*self)
    }
}

/// A closed enum spelled by name: each name beside its value, so reading and writing cannot
/// drift, and the two arrays cannot differ in length.
#[derive(Debug)]
struct Names<T: 'static, const N: usize> {
    names: [&'static str; N],
    values: [T; N],
}

/// `layout = "..."`.
const LAYOUTS: Names<Layout, 4> = Names {
    names: ["blog", "vault", "docs", "flat"],
    values: [Layout::Blog, Layout::Vault, Layout::Docs, Layout::Flat],
};

/// `kind = "..."`.
const KINDS: Names<PageKind, 3> =
    Names { names: ["home", "post", "page"], values: [PageKind::Home, PageKind::Post, PageKind::Page] };

impl<T: Copy + PartialEq, const N: usize> Names<T, N> {
    /// The value `node` names.
    fn read(&self, node: Node<'_>) -> Result<T, Refused> {
        let text = node.text()?;
        self.names
            .iter()
            .zip(self.values)
            .find(|(name, _)| **name == text)
            .map(|(_, value)| value)
            .ok_or_else(|| node.refused())
    }

    /// The name of `value`; [`Unwritable::Unknown`] for a value the model added after this table.
    fn write(&self, value: T) -> Result<Written, Unwritable> {
        self.names
            .iter()
            .zip(self.values)
            .find(|(_, v)| *v == value)
            .map(|(name, _)| (*name).into())
            .ok_or(Unwritable::Unknown)
    }
}
