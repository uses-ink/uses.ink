//! The fields of [`Front`]: for each, the keys that set it, the value type that reads it, and
//! where in `Front` it is held. A keyed code is a field through its own [`Keyed::KEYS`] and
//! [`Code::Value`]; `draft`, `lang` and `aliases` are fields no file-scoped code is keyed by.

use model::code::{self, Code, Keyed};
use model::{FromFront, Front, Language, PageKind, Slug, Spanned, Tag, When};

/// One field of [`Front`].
pub(crate) trait Field {
    /// The keys that set it, in precedence order; the first written wins.
    const KEYS: &'static [&'static str];
    /// What a key's value is read as.
    type Value: FromFront;
    /// What `Front` holds.
    type Held;

    /// The value as `Front` holds it, or `None` when `Front` cannot hold it: an explicit none
    /// (`date: false`) has no place there yet, so it is refused rather than dropped.
    fn hold(value: Self::Value) -> Option<Self::Held>;

    /// Where `Front` holds it.
    fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Held>>;
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
    type Held = Vec<String>;

    fn hold(value: Vec<String>) -> Option<Vec<String>> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<Vec<String>>> {
        &mut front.aliases
    }
}

impl Field for code::Title {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = String;

    fn hold(value: String) -> Option<String> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<String>> {
        &mut front.title
    }
}

impl Field for code::Date {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = When;

    fn hold(value: Option<When>) -> Option<When> {
        value
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<When>> {
        &mut front.date
    }
}

impl Field for code::Updated {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = When;

    fn hold(value: Option<When>) -> Option<When> {
        value
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<When>> {
        &mut front.updated
    }
}

impl Field for code::Slug {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = Slug;

    fn hold(value: Slug) -> Option<Slug> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<Slug>> {
        &mut front.slug
    }
}

impl Field for code::Tags {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = Vec<Tag>;

    fn hold(value: Vec<Tag>) -> Option<Vec<Tag>> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<Vec<Tag>>> {
        &mut front.tags
    }
}

impl Field for code::Publish {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = bool;

    fn hold(value: bool) -> Option<bool> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<bool>> {
        &mut front.publish
    }
}

impl Field for Draft {
    const KEYS: &'static [&'static str] = &["draft"];
    type Value = bool;
    type Held = bool;

    fn hold(value: bool) -> Option<bool> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<bool>> {
        &mut front.draft
    }
}

impl Field for code::Kind {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = PageKind;

    fn hold(value: PageKind) -> Option<PageKind> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<PageKind>> {
        &mut front.kind
    }
}

impl Field for code::Summary {
    const KEYS: &'static [&'static str] = <Self as Keyed>::KEYS;
    type Value = <Self as Code>::Value;
    type Held = String;

    fn hold(value: Option<String>) -> Option<String> {
        value
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<String>> {
        &mut front.description
    }
}

impl Field for Lang {
    const KEYS: &'static [&'static str] = &["lang"];
    type Value = Language;
    type Held = Language;

    fn hold(value: Language) -> Option<Language> {
        Some(value)
    }

    fn slot(front: &mut Front) -> &mut Option<Spanned<Language>> {
        &mut front.language
    }
}
