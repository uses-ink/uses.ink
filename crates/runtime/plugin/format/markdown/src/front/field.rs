//! The fields of [`Front`]: for each, the keys that set it, the value type that reads it, and
//! where in `Front` it is held. A keyed code is a field through its own [`Keyed::KEYS`] and
//! [`Code::Value`]; `draft`, `lang` and `aliases` are fields no file-scoped code is keyed by.

use model::code::{self, Code, Keyed};
use model::{FromFront, Front, Language, Spanned};

/// One field of [`Front`].
pub(crate) trait Field {
    /// The keys that set it, in precedence order; the first written wins.
    const KEYS: &'static [&'static str];
    /// What a key's value is read as, and what `Front` holds: an optional value holds its none
    /// (`date: false`) as a decided none.
    type Value: FromFront;

    /// Where `Front` holds it.
    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>>;
}

/// `draft:`, which the engine's `page.publish` reads as its opposite.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Draft;

/// `lang:`, the page's language, which the engine reads off the home page for `site.language`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lang;

/// `aliases:` (or `alias:`), other names a link may use for the page.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Aliases;

impl Field for Aliases {
    const KEYS: &'static [&'static str] = &["aliases", "alias"];
    type Value = Vec<String>;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.aliases
    }
}

impl Field for code::Title {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.title
    }
}

impl Field for code::Date {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.date
    }
}

impl Field for code::Updated {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.updated
    }
}

impl Field for code::Slug {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.slug
    }
}

impl Field for code::Tags {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.tags
    }
}

impl Field for code::Publish {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.publish
    }
}

impl Field for Draft {
    const KEYS: &'static [&'static str] = &["draft"];
    type Value = bool;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.draft
    }
}

impl Field for code::Kind {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.kind
    }
}

impl Field for code::Summary {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.description
    }
}

impl Field for Lang {
    const KEYS: &'static [&'static str] = &["lang"];
    type Value = Language;

    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
        &mut front.language
    }
}
