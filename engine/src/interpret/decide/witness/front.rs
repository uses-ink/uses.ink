//! What a page says of itself: its frontmatter keys ([`Front`]), its `draft` key ([`Draft`]) and
//! its first level-1 heading ([`Heading`]).

use std::marker::PhantomData;

use model::code::{Code, Date, Kind, Language, Publish, Slug, Summary, Tags, Title, Updated};
use model::{Evidence, Span};

use crate::interpret::decide::Sourced;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};
use crate::interpret::decide::trace::{Named, Witnessed};

/// A code a page's frontmatter can say. The format reads the keys into `model::Front`; each code
/// takes its own field.
pub(crate) trait Fronted: Code {
    /// The value the frontmatter gives, and where it is written.
    fn front(front: &model::Front) -> Option<(Self::Value, Span)>;
}

/// The frontmatter of the page the ground holds: the file itself, or the site's home.
#[derive(Debug, Default)]
pub(crate) struct Front<C>(PhantomData<C>);

impl<C: Fronted, G: Sourced + ?Sized> Witness<C, G> for Front<C> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        let Some(source) = case.ground.source() else { return };
        if let Some((value, span)) = C::front(&source.read.front) {
            testimony.value(value, source.front(span));
        }
    }
}

/// Implements [`Fronted`] for each code from its field of `model::Front`, wrapping the value
/// where the code's value is optional. The same impl over a list of types, which a generic
/// cannot write.
///
/// ```text
/// fronted! { Title => title; Date => date as Some; }
/// // makes `impl Fronted for Date`, reading `front.date` as `Some(date)`
/// ```
macro_rules! fronted {
    ($($code:ty => $field:ident $(as $wrap:ident)?;)+) => {
        $(
            impl Fronted for $code {
                fn front(front: &model::Front) -> Option<(Self::Value, Span)> {
                    let written = front.$field.as_ref()?;
                    Some(($($wrap)?(written.value.clone()), written.span))
                }
            }
        )+
    };
}

fronted! {
    Kind => kind;
    Publish => publish;
    Title => title;
    Summary => description as Some;
    Date => date as Some;
    Updated => updated as Some;
    Slug => slug;
    Tags => tags;
    Language => language;
}

/// `draft: true`, which means not published. It only ever testifies against publishing:
/// `draft: false` says nothing, so it never selects a page of a vault or a private source
/// (decision 0017), where only `publish: true` or `include` does.
#[derive(Debug, Default)]
pub(crate) struct Draft;

impl<G: Sourced + ?Sized> Witness<Publish, G> for Draft {
    fn testify(&self, case: &Case<'_, Publish, G>, testimony: &mut Testimony<bool>) {
        let Some(source) = case.ground.source() else { return };
        if let Some(draft) = source.read.front.draft.as_ref().filter(|draft| draft.value) {
            testimony.value(false, source.front(draft.span));
        }
    }
}

/// The page's first level-1 heading, as its title.
#[derive(Debug, Default)]
pub(crate) struct Heading;

impl<G: Sourced + ?Sized> Witness<Title, G> for Heading {
    fn testify(&self, case: &Case<'_, Title, G>, testimony: &mut Testimony<String>) {
        let Some(source) = case.ground.source() else { return };
        if let Some(heading) = &source.read.heading {
            let evidence = Evidence::Heading { path: source.path.clone(), span: heading.span };
            testimony.value(heading.value.clone(), evidence);
        }
    }
}

impl<C> Named for Front<C> {
    const NAME: Witnessed = Witnessed::Front;
}

impl Named for Draft {
    const NAME: Witnessed = Witnessed::Draft;
}

impl Named for Heading {
    const NAME: Witnessed = Witnessed::Heading;
}
