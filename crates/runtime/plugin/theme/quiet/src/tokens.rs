//! The tokens `ink:theme-quiet` declares, set in a site's `[theme]` table: which [`Look`] it
//! wears and which [`Navigation`] it draws. Each token is a word out of a closed set, so a
//! value the theme does not know is refused with the words it does, never ignored.
//!
//! ```toml
//! [theme]
//! use = "ink:theme-quiet"
//! look = "uses-ink"        # the palette, faces and sizes of `crates/core/token/token.toml`
//! navigation = "docs"      # a sidebar, previous and next links, an outline of the page
//! ```

use std::fmt;

use model::{Diagnostic, Problem, Spanned};

/// A token's words, each beside the value it means.
trait Word: Copy + Default + PartialEq + 'static {
    /// The token's name, as `[theme]` writes it.
    const TOKEN: &'static str;
    /// Every value, by the word that sets it.
    const WORDS: &'static [(&'static str, Self)];

    /// The value `word` sets, or the refusal that lists the words.
    fn parse(word: &str) -> Result<Self, Refused> {
        Self::WORDS.iter().find(|(known, _)| *known == word).map(|(_, value)| *value).ok_or_else(|| Refused::Invalid {
            token: Self::TOKEN,
            value: word.to_owned(),
            accepted: Self::WORDS.iter().map(|(known, _)| *known).collect(),
        })
    }

    /// The words, which are what a declaration lists.
    fn words() -> Vec<&'static str> {
        Self::WORDS.iter().map(|(word, _)| *word).collect()
    }

    /// The word that sets this value.
    fn word(self) -> &'static str {
        Self::WORDS.iter().find(|(_, value)| *value == self).map_or("", |(word, _)| *word)
    }
}

/// The palette, faces and sizes the pages are written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Look {
    /// The theme's own: system faces, a warm paper, a blue link.
    #[default]
    Quiet,
    /// The uses.ink look of `crates/core/token/token.toml`: the documentation palette and type.
    UsesInk,
}

impl Word for Look {
    const TOKEN: &'static str = "look";
    const WORDS: &'static [(&'static str, Self)] = &[("quiet", Self::Quiet), ("uses-ink", Self::UsesInk)];
}

/// What is drawn around a page besides its header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Navigation {
    /// The header's links and nothing else.
    #[default]
    Header,
    /// A sidebar of the site's folders, previous and next links, and an outline of the page.
    Docs,
}

impl Word for Navigation {
    const TOKEN: &'static str = "navigation";
    const WORDS: &'static [(&'static str, Self)] = &[("header", Self::Header), ("docs", Self::Docs)];
}

/// What a site set for the theme: every token, each at its default until it says otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Tokens {
    /// The palette, faces and sizes.
    pub look: Look,
    /// What is drawn around a page.
    pub navigation: Navigation,
}

/// One token the theme declares: its name, the words it takes and the one it has by default.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Declared {
    /// As `[theme]` writes it.
    pub name: &'static str,
    /// The words it takes.
    pub accepts: Vec<&'static str>,
    /// What it is when unset.
    pub default: &'static str,
}

/// Why a token was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// The theme has no such token.
    Unknown {
        /// As written.
        token: String,
        /// The tokens it has.
        known: Vec<&'static str>,
    },
    /// The token takes other words.
    Invalid {
        /// The token.
        token: &'static str,
        /// As written.
        value: String,
        /// The words it takes.
        accepted: Vec<&'static str>,
    },
}

impl Tokens {
    /// The name the theme goes by in a diagnostic.
    const THEME: &'static str = "ink:theme-quiet";

    /// Every token the theme declares, in the order a site is told of them.
    pub fn declared() -> Vec<Declared> {
        vec![
            Declared { name: Look::TOKEN, accepts: Look::words(), default: Look::default().word() },
            Declared { name: Navigation::TOKEN, accepts: Navigation::words(), default: Navigation::default().word() },
        ]
    }

    /// These tokens with `name` set to `value`.
    ///
    /// # Errors
    ///
    /// [`Refused`] when the theme declares no token `name`, or `value` is not one of its words.
    pub fn set(self, name: &str, value: &str) -> Result<Self, Refused> {
        match name {
            Look::TOKEN => Ok(Self { look: Look::parse(value)?, ..self }),
            Navigation::TOKEN => Ok(Self { navigation: Navigation::parse(value)?, ..self }),
            _ => Err(Refused::Unknown { token: name.to_owned(), known: vec![Look::TOKEN, Navigation::TOKEN] }),
        }
    }

    /// The tokens `pairs` set, name and value each, the first refusal if any is.
    ///
    /// # Errors
    ///
    /// The first [`Refused`] of the pairs.
    pub fn read<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<Self, Refused> {
        pairs.into_iter().try_fold(Self::default(), |tokens, (name, value)| tokens.set(name, value))
    }

    /// The tokens a site wrote, each name and word with the place it was written, or a diagnostic
    /// for every one the theme refuses: at the name when the theme has no such token, at the
    /// word when it takes other words. The caller says which file the places are in.
    ///
    /// # Errors
    ///
    /// One [`Diagnostic`] for each token refused.
    pub fn gather<'a>(
        written: impl IntoIterator<Item = (Spanned<&'a str>, Spanned<&'a str>)>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let (mut tokens, mut refused) = (Self::default(), Vec::new());
        for (name, value) in written {
            match tokens.set(name.value, value.value) {
                Ok(set) => tokens = set,
                Err(why) => {
                    let at = if matches!(why, Refused::Invalid { .. }) { value.span } else { name.span };
                    refused.push(Diagnostic::new(why.problem()).at(at));
                }
            }
        }
        if refused.is_empty() { Ok(tokens) } else { Err(refused) }
    }

    /// Each token with the word that sets it, in declaration order: what a theme's identity and
    /// a cache key are made from.
    pub fn words(self) -> [(&'static str, &'static str); 2] {
        [(Look::TOKEN, self.look.word()), (Navigation::TOKEN, self.navigation.word())]
    }
}

impl Refused {
    /// The diagnostic's problem, for the theme `ink:theme-quiet`.
    pub fn problem(&self) -> Problem {
        let words = |words: &[&str]| words.iter().map(|word| format!("`{word}`")).collect::<Vec<_>>().join(", ");
        let theme = Tokens::THEME.to_owned();
        match self {
            Self::Unknown { token, known } => {
                Problem::ThemeTokenUnknown { theme, token: token.clone(), known: words(known) }
            }
            Self::Invalid { token, value, accepted } => Problem::ThemeTokenInvalid {
                theme,
                token: (*token).to_owned(),
                value: value.clone(),
                accepted: words(accepted),
            },
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { token, known } => write!(out, "`{token}` is not a token of the theme; it has {known:?}"),
            Self::Invalid { token, value, accepted } => {
                write!(out, "`{value}` is not a value of `{token}`; it takes {accepted:?}")
            }
        }
    }
}

impl std::error::Error for Refused {}
