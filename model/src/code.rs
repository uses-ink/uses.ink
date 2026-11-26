//! Decision codes as types. A code fixes its name (the `ink.toml` path that pins it), its value
//! type and its subject kind, so a [`Decision<Title>`](Title) holds a `String` about a file and
//! nothing else. Code that decides is generic over [`C: Code`](Code); only storage and the wire
//! see the erased [`AnyDecision`] and [`AnyCode`].
//!
//! The closed set of codes is declared once, by `codes!` at the bottom of this file.

use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::decision::decided::{self, Decided, DecisionCode};
use crate::decision::{Alternative, Decision, Evidence, File, Mismatch, Scope, Site, Subject};
use crate::value::{FromFront, Value};
use crate::{Author, ContentPath, PageKind, Tag, When};

/// A decision code: a zero-sized type naming one kind of decision. Implemented by `codes!` for
/// every code of the closed set, never by hand.
pub trait Code: Copy + Ord + Hash + Default + fmt::Debug + Send + Sync + 'static {
    /// `table.key`, the pin's path in `ink.toml` (decision 0018): `page.title`.
    const NAME: &'static str;
    /// The code, erased.
    const CODE: AnyCode;
    /// What it decides.
    type Value: Value;
    /// What it decides about: [`Site`] or [`File`].
    type Subject: Subject;

    /// The decision, erased.
    fn erase(decision: Decision<Self>) -> AnyDecision;

    /// The decision, when `decision` is one of this code; else `decision` back.
    fn claim(decision: AnyDecision) -> Result<Decision<Self>, AnyDecision>;

    /// A view of the decision, when `decision` is one of this code.
    fn view(decision: &AnyDecision) -> Option<&Decision<Self>>;
}

/// A code an author can also decide in a file's frontmatter, under one of its keys. The first
/// key written wins; a format reports the others.
pub trait Keyed: Code<Subject = File, Value: FromFront> {
    /// The frontmatter keys, in precedence order: `["description", "summary"]`.
    const KEYS: &'static [&'static str];
}

impl<C: Code> From<Decision<C>> for AnyDecision {
    fn from(decision: Decision<C>) -> Self {
        C::erase(decision)
    }
}

impl<C: Code> TryFrom<AnyDecision> for Decision<C> {
    type Error = Mismatch;

    fn try_from(decision: AnyDecision) -> Result<Self, Self::Error> {
        C::claim(decision).map_err(|other| Mismatch::Code { expected: C::CODE, found: other.code() })
    }
}

impl AnyDecision {
    /// The decision as code `C`, when it is one.
    pub fn get<C: Code>(&self) -> Option<&Decision<C>> {
        C::view(self)
    }
}

impl fmt::Display for AnyCode {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for AnyCode {
    type Err = UnknownDecision;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|code| code.name() == name)
            .ok_or_else(|| UnknownDecision { name: name.to_owned() })
    }
}

impl TryFrom<String> for AnyCode {
    type Error = UnknownDecision;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        name.parse()
    }
}

impl From<AnyCode> for &'static str {
    fn from(code: AnyCode) -> Self {
        code.name()
    }
}

/// A name that is no decision code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownDecision {
    /// The name as given.
    pub name: String,
}

impl fmt::Display for UnknownDecision {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is no decision code", self.name)
    }
}

impl std::error::Error for UnknownDecision {}

/// Declares the closed set of decision codes, in the order the inferrers decide them
/// (architecture.md section 9), and generates from it:
///
/// - one zero-sized type per code, implementing [`Code`] (and [`Keyed`] when it has `keys`);
/// - [`AnyCode`], the erased code, with [`AnyCode::ALL`], its name and its scope;
/// - [`AnyDecision`], the erased decision, serialized with the code's name as its `code` tag;
/// - the conversions between [`AnyDecision`] and the untyped [`decided::Decision`], and
///   between [`AnyCode`] and [`DecisionCode`], until those go.
///
/// The same impls over a list of types, which a generic cannot write.
///
/// ```text
/// codes! {
///     /// A page's title.
///     Title => "page.title" on File: String, keys ["title"];
/// }
/// // makes `code::Title`, whose `Value` is `String` and whose `Subject` is `File`
/// ```
macro_rules! codes {
    ($(
        $(#[$doc:meta])*
        $name:ident => $code:literal on $subject:ident: $value:ty $(, keys [$($key:literal),+])?;
    )*) => {
        $(
            $(#[$doc])*
            #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub struct $name;

            impl Code for $name {
                const NAME: &'static str = $code;
                const CODE: AnyCode = AnyCode::$name;
                type Value = $value;
                type Subject = $subject;

                fn erase(decision: Decision<Self>) -> AnyDecision {
                    AnyDecision::$name(decision)
                }

                fn claim(decision: AnyDecision) -> Result<Decision<Self>, AnyDecision> {
                    match decision {
                        AnyDecision::$name(decision) => Ok(decision),
                        other => Err(other),
                    }
                }

                fn view(decision: &AnyDecision) -> Option<&Decision<Self>> {
                    match decision {
                        AnyDecision::$name(decision) => Some(decision),
                        _ => None,
                    }
                }
            }

            $(
                impl Keyed for $name {
                    const KEYS: &'static [&'static str] = &[$($key),+];
                }
            )?
        )*

        /// A decision code, erased: one variant per code type, written as its name.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(into = "&'static str", try_from = "String")]
        #[non_exhaustive]
        pub enum AnyCode {
            $(
                $(#[$doc])*
                $name,
            )*
        }

        impl AnyCode {
            /// Every code, in the order the inferrers decide them.
            pub const ALL: &'static [Self] = &[$(Self::$name),*];

            /// `table.key`, the pin's path in `ink.toml`.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$name => $code,)*
                }
            }

            /// Whether it is decided once for the site or once per file.
            pub const fn scope(self) -> Scope {
                match self {
                    $(Self::$name => <$subject as Subject>::SCOPE,)*
                }
            }
        }

        /// A decision of any code: the form storage, `ink explain` and the plugin boundary hold.
        /// Serialized as the typed decision's fields with a `code` field naming its code.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "code")]
        #[non_exhaustive]
        pub enum AnyDecision {
            $(
                $(#[$doc])*
                #[serde(rename = $code)]
                $name(Decision<$name>),
            )*
        }

        impl AnyDecision {
            /// Its code.
            pub const fn code(&self) -> AnyCode {
                match self {
                    $(Self::$name(_) => AnyCode::$name,)*
                }
            }

            /// What it is about, erased.
            pub fn subject(&self) -> crate::Subject {
                match self {
                    $(Self::$name(decision) => decision.subject.clone().into(),)*
                }
            }

            /// Why it chose its value.
            pub const fn evidence(&self) -> &Evidence {
                match self {
                    $(Self::$name(decision) => &decision.evidence,)*
                }
            }
        }

        impl From<DecisionCode> for AnyCode {
            fn from(code: DecisionCode) -> Self {
                match code {
                    $(DecisionCode::$name => Self::$name,)*
                }
            }
        }

        impl From<AnyCode> for DecisionCode {
            fn from(code: AnyCode) -> Self {
                match code {
                    $(AnyCode::$name => Self::$name,)*
                }
            }
        }

        impl TryFrom<decided::Decision> for AnyDecision {
            type Error = Mismatch;

            /// Refuses a subject of the wrong scope and a rejected value of another code, which
            /// the untyped decision can hold and the typed one cannot.
            fn try_from(old: decided::Decision) -> Result<Self, Self::Error> {
                let decided::Decision { decided, subject, evidence, rejected } = old;
                match decided {
                    $(
                        Decided::$name(value) => {
                            let rejected = rejected
                                .into_iter()
                                .map(|lost| match lost.decided {
                                    Decided::$name(value) => Ok(Alternative::new(value, lost.evidence, lost.because)),
                                    other => Err(Mismatch::Code { expected: AnyCode::$name, found: other.code().into() }),
                                })
                                .collect::<Result<_, _>>()?;
                            let subject = subject.try_into()?;
                            Ok(Self::$name(Decision { subject, value, evidence, rejected }))
                        }
                    )*
                }
            }
        }

        impl From<AnyDecision> for decided::Decision {
            fn from(decision: AnyDecision) -> Self {
                match decision {
                    $(
                        AnyDecision::$name(Decision { subject, value, evidence, rejected }) => {
                            let rejected = rejected
                                .into_iter()
                                .map(|lost| decided::Alternative::new(Decided::$name(lost.value), lost.evidence, lost.because))
                                .collect();
                            Self { decided: Decided::$name(value), subject: subject.into(), evidence, rejected }
                        }
                    )*
                }
            }
        }
    };
}

codes! {
    /// `site.root`: where the site's content starts in the snapshot.
    Root => "site.root" on Site: crate::Root;
    /// `site.layout`: how the site's files are arranged.
    Layout => "site.layout" on Site: crate::Layout;
    /// `file.ignore`: whether a file is left out of the site.
    Ignored => "file.ignore" on File: bool;
    /// `page.kind`: what a page is to its site.
    Kind => "page.kind" on File: PageKind, keys ["type"];
    /// `page.publish`: whether a page is published.
    Publish => "page.publish" on File: bool, keys ["publish"];
    /// `page.title`: a page's title.
    Title => "page.title" on File: String, keys ["title"];
    /// `page.summary`: a page's summary for feeds, listings and `og:description`, or none when
    /// it has no text.
    Summary => "page.summary" on File: Option<String>, keys ["description", "summary"];
    /// `page.date`: when a page was written, or none.
    Date => "page.date" on File: Option<When>, keys ["date"];
    /// `page.updated`: when a page was last changed, or never.
    Updated => "page.updated" on File: Option<When>, keys ["updated"];
    /// `page.slug`: the last segment of a page's route.
    Slug => "page.slug" on File: crate::Slug, keys ["slug"];
    /// `page.tags`: a page's tags.
    Tags => "page.tags" on File: Vec<Tag>, keys ["tags"];
    /// `site.home`: the home page's file, or none for a generated listing.
    Home => "site.home" on Site: Option<ContentPath>;
    /// `site.nav`: the pages in the navigation, in order.
    Nav => "site.nav" on Site: Vec<ContentPath>;
    /// `site.title`: the site's name, shown in every page's title and its feeds.
    SiteTitle => "site.title" on Site: String;
    /// `site.description`: one line saying what the site is, or none.
    Description => "site.description" on Site: Option<String>;
    /// `site.authors`: who wrote the site, first author first.
    Authors => "site.authors" on Site: Vec<Author>;
    /// `site.language`: the language of the site's pages.
    Language => "site.language" on Site: crate::Language;
}
