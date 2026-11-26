//! The `[theme]` table: which theme plugin renders the site, and the tokens set for it.
//!
//! ```toml
//! [theme]
//! use = "ink:theme-quiet"
//! accent = "#2b59c3"
//! ```
//!
//! `use` names the plugin; leaving it out keeps the default theme, and the engine says so with a
//! notice. Every other key is a token. Which tokens a theme has is the theme's to declare, so
//! the table accepts any kebab-case key here and the engine checks it against the theme.

use std::fmt;
use std::str::FromStr;

use model::{Span, Spanned};

/// The theme choice and its tokens, each with its span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// `use = "..."`, or `None` for the default theme.
    pub plugin: Option<Spanned<ThemeId>>,
    /// In the order written.
    pub tokens: Vec<Token>,
    /// The `[theme]` table itself.
    pub span: Span,
}

/// One token: `accent = "#2b59c3"`. Values are strings until themes declare typed tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// The token's kebab-case name, the key.
    pub name: Spanned<String>,
    /// What it is set to.
    pub value: Spanned<String>,
}

/// A theme plugin's id: `<scope>:theme-<variant>`, as in `ink:theme-quiet` (conventions.md,
/// Grammar). Scope and variant are kebab-case: lower-case letters and digits, with single `-`
/// inside.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThemeId {
    scope: String,
    variant: String,
}

/// Why text is not a [`ThemeId`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ThemeIdError {
    /// No `:` between scope and name.
    NoScope,
    /// The name after `:` does not start with `theme-`.
    NotTheme,
    /// The scope or the variant is not kebab-case.
    NotKebab(IdPart),
}

/// A part of a [`ThemeId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdPart {
    /// `ink` in `ink:theme-quiet`.
    Scope,
    /// `quiet` in `ink:theme-quiet`.
    Variant,
}

impl fmt::Display for ThemeIdError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoScope => write!(out, "a plugin id is `<scope>:<slot>-<variant>`"),
            Self::NotTheme => write!(out, "the plugin is not a theme: its name starts with `theme-`"),
            Self::NotKebab(IdPart::Scope) => write!(out, "the scope is lower-case letters, digits and single `-`"),
            Self::NotKebab(IdPart::Variant) => {
                write!(out, "the variant is lower-case letters, digits and single `-`")
            }
        }
    }
}

impl std::error::Error for ThemeIdError {}

impl FromStr for ThemeId {
    type Err = ThemeIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (scope, name) = text.split_once(':').ok_or(ThemeIdError::NoScope)?;
        let variant = name.strip_prefix("theme-").ok_or(ThemeIdError::NotTheme)?;
        let Kebab(scope) = Kebab::try_from(scope).map_err(|NotKebab| ThemeIdError::NotKebab(IdPart::Scope))?;
        let Kebab(variant) = Kebab::try_from(variant).map_err(|NotKebab| ThemeIdError::NotKebab(IdPart::Variant))?;
        Ok(Self { scope: scope.to_owned(), variant: variant.to_owned() })
    }
}

impl ThemeId {
    /// `ink` in `ink:theme-quiet`.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// `quiet` in `ink:theme-quiet`.
    pub fn variant(&self) -> &str {
        &self.variant
    }
}

impl fmt::Display for ThemeId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}:theme-{}", self.scope, self.variant)
    }
}

/// Text that is kebab-case: lower-case letters and digits, with single `-` inside, as in
/// `theme-quiet` and `accent-2`. A token's name and both parts of a [`ThemeId`] are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Kebab<'a>(&'a str);

/// Text that is not kebab-case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NotKebab;

impl<'a> TryFrom<&'a str> for Kebab<'a> {
    type Error = NotKebab;

    fn try_from(text: &'a str) -> Result<Self, Self::Error> {
        let kebab = !text.is_empty()
            && !text.starts_with('-')
            && !text.ends_with('-')
            && !text.contains("--")
            && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if kebab { Ok(Self(text)) } else { Err(NotKebab) }
    }
}

#[cfg(test)]
mod test {
    use super::{IdPart, ThemeId, ThemeIdError};

    #[test]
    fn reads_an_id() {
        let id = str::parse::<ThemeId>("ink:theme-quiet").unwrap();
        assert_eq!((id.scope(), id.variant(), id.to_string().as_str()), ("ink", "quiet", "ink:theme-quiet"));
        assert_eq!(str::parse::<ThemeId>("bob:theme-zine-2").unwrap().variant(), "zine-2");
    }

    #[test]
    fn refuses_each_bad_shape() {
        let refused = |text: &str| str::parse::<ThemeId>(text).unwrap_err();
        assert_eq!(refused("theme-quiet"), ThemeIdError::NoScope);
        assert_eq!(refused("ink:format-markdown"), ThemeIdError::NotTheme);
        assert_eq!(refused("Ink:theme-quiet"), ThemeIdError::NotKebab(IdPart::Scope));
        assert_eq!(refused("ink:theme-"), ThemeIdError::NotKebab(IdPart::Variant));
        assert_eq!(refused("ink:theme-a--b"), ThemeIdError::NotKebab(IdPart::Variant));
    }
}
