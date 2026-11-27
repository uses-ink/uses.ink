//! [`Fallback`]: the default, when nothing else testified. Each code says what its default is
//! on each ground through [`Assumed`].

use std::fmt;
use std::marker::PhantomData;

use model::code::{
    Authors, Date, Description, Home, Ignored, Kind, Language, Layout, Nav, Publish, Root, SiteTitle, Slug, Summary,
    Tags, Title, Updated,
};
use model::decision::File;
use model::{DefaultReason, Evidence, PageId, PageKind};

use crate::error::Error;
use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};
use crate::interpret::decide::trace::{Named, Witnessed};
use crate::interpret::file::asset;
use crate::interpret::page::{self, Selection};
use crate::interpret::site;

/// What a code is taken to be on ground `G` when nothing testifies to it, and why.
pub(crate) trait Assumed<G: ?Sized>: model::code::Code {
    /// The default and the reason it is the default.
    fn assumed(case: &Case<'_, Self, G>) -> Result<(Self::Value, DefaultReason), Error>;
}

/// The default, evidence `Default { reason }`, when no witness before it said a value. When
/// only bounds were said, the reason is [`DefaultReason::OnlyBound`].
#[derive(Debug, Default)]
pub(crate) struct Fallback<C>(PhantomData<C>);

impl<C: Assumed<G>, G: ?Sized> Witness<C, G> for Fallback<C> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        if testimony.is_decided() {
            return;
        }
        match C::assumed(case) {
            Ok((value, reason)) => {
                let reason = if testimony.is_bounded() { DefaultReason::OnlyBound } else { reason };
                testimony.value(value, Evidence::Default { reason });
            }
            Err(error) => testimony.fail(error),
        }
    }
}

/// Implements [`Assumed`] on every ground for codes whose default is one value. The same impl
/// over a list of types, which a generic cannot write.
///
/// ```text
/// constant! { Root => model::Root::Top; }
/// // makes `impl<G: ?Sized> Assumed<G> for Root`, assuming the top of the snapshot
/// ```
macro_rules! constant {
    ($($code:ty => $value:expr;)+) => {
        $(
            impl<G: ?Sized> Assumed<G> for $code {
                fn assumed(_case: &Case<'_, Self, G>) -> Result<(Self::Value, DefaultReason), Error> {
                    Ok(($value, DefaultReason::NothingMatched))
                }
            }
        )+
    };
}

constant! {
    Root => model::Root::Top;
    Layout => model::Layout::Flat;
    Ignored => false;
    Kind => PageKind::Page;
    Summary => None;
    Date => None;
    Updated => None;
    Tags => Vec::new();
    Home => None;
    SiteTitle => Untitled.to_string();
    Description => None;
    Authors => Vec::new();
}

/// The title of a site nothing names.
#[derive(Clone, Copy, Debug)]
struct Untitled;

impl fmt::Display for Untitled {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Untitled site")
    }
}

impl<G: Filed + ?Sized> Assumed<G> for Slug {
    /// The page id's first digits, for a name with no letter or digit.
    fn assumed(case: &Case<'_, Self, G>) -> Result<(model::Slug, DefaultReason), Error> {
        Ok((PageId::of(case.ground.path()).slug(), DefaultReason::NothingMatched))
    }
}

impl Assumed<page::Ground<'_>> for Publish {
    /// Published, except in a vault or a private source, where nothing is published until it
    /// is selected (decision 0017).
    fn assumed(case: &Case<'_, Self, page::Ground<'_>>) -> Result<(bool, DefaultReason), Error> {
        Ok(match case.ground.selection {
            Selection::Implicit => (true, DefaultReason::NothingMatched),
            Selection::Explicit => (false, DefaultReason::NotSelected),
        })
    }
}

impl Assumed<asset::Ground<'_>> for Publish {
    /// A plain file no published page uses is not published.
    fn assumed(_case: &Case<'_, Self, asset::Ground<'_>>) -> Result<(bool, DefaultReason), Error> {
        Ok((false, DefaultReason::NothingMatched))
    }
}

impl<G: ?Sized> Assumed<G> for Language {
    /// English.
    fn assumed(_case: &Case<'_, Self, G>) -> Result<(model::Language, DefaultReason), Error> {
        const WRITTEN: &str = "en";
        let language = WRITTEN.parse().map_err(|source| Error::Language { written: WRITTEN, source })?;
        Ok((language, DefaultReason::NothingMatched))
    }
}

impl Assumed<site::Ground<'_>> for Nav {
    /// Every published page at the top of the root that is neither a post nor the home, by
    /// title then path.
    fn assumed(case: &Case<'_, Self, site::Ground<'_>>) -> Result<(Vec<model::ContentPath>, DefaultReason), Error> {
        let ground = case.ground;
        let home = ground.home.map(|home| &home.path);
        let mut listed: Vec<(Option<&String>, &model::ContentPath)> = ground
            .sources
            .iter()
            .filter(|source| source.relative.parent().is_none() && ground.published.contains(&source.path))
            .filter(|source| Some(&source.path) != home)
            .filter_map(|source| {
                let file = File(source.path.clone());
                let post = ground.journal.value::<Kind>(&file) == Some(&PageKind::Post);
                (!post).then(|| (ground.journal.value::<Title>(&file), &source.path))
            })
            .collect();
        listed.sort();
        Ok((listed.into_iter().map(|(_, path)| path.clone()).collect(), DefaultReason::NothingMatched))
    }
}

impl<C> Named for Fallback<C> {
    const NAME: Witnessed = Witnessed::Default;
}
