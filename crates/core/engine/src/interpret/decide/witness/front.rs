//! What a page says of itself: its frontmatter keys ([`Front`]), what its text adds to them
//! ([`Inline`]), its `draft` key ([`Draft`]) and its first level-1 heading ([`Heading`]).

use std::marker::PhantomData;

use model::code::{Code, Date, Kind, Language, Publish, Slug, Summary, Tags, Title, Updated};
use model::{Evidence, Span, Spanned, Tag};

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

/// Implements [`Fronted`] for each code from its field of `model::Front`. The same impl over a
/// list of types, which a generic cannot write.
///
/// ```text
/// fronted! { Title => title; Date => date; }
/// // makes `impl Fronted for Date`, reading `front.date`, whose none (`date: false`) is a value
/// ```
macro_rules! fronted {
    ($($code:ty => $field:ident;)+) => {
        $(
            impl Fronted for $code {
                fn front(front: &model::Front) -> Option<(Self::Value, Span)> {
                    let written = front.$field.as_ref()?;
                    Some((written.value.clone(), written.span))
                }
            }
        )+
    };
}

fronted! {
    Kind => kind;
    Publish => publish;
    Title => title;
    Summary => description;
    Date => date;
    Updated => updated;
    Slug => slug;
    Tags => tags;
    Language => language;
}

/// A code a page's text can add to what its frontmatter says: Obsidian's `#tag`.
pub(crate) trait Inlined: Code {
    /// The frontmatter's value with what the text adds to it, and where the first addition is
    /// written; nothing when the text adds nothing.
    fn inline(read: &model::Read) -> Option<(Self::Value, Span)>;
}

impl Inlined for Tags {
    /// The frontmatter's tags, then the text's that are not among them, each slug once.
    fn inline(read: &model::Read) -> Option<(Vec<Tag>, Span)> {
        let mut tags = read.front.tags.as_ref().map_or_default(|written| written.value.clone());
        let mut first = None;
        for Spanned { value, span } in &read.inline_tags {
            if !tags.iter().any(|tag| tag.slug == value.slug) {
                tags.push(value.clone());
                first.get_or_insert(*span);
            }
        }
        first.map(|span| (tags, span))
    }
}

/// What the page's text adds to its frontmatter: the two joined, since Obsidian shows both. It
/// testifies only when the text adds something, so it stands before [`Front`]: a page whose
/// text adds nothing is decided by its frontmatter, and the evidence is the text's whenever the
/// value holds something only the text said.
#[derive(Debug, Default)]
pub(crate) struct Inline<C>(PhantomData<C>);

impl<C: Inlined, G: Sourced + ?Sized> Witness<C, G> for Inline<C> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        let Some(source) = case.ground.source() else { return };
        if let Some((value, span)) = C::inline(&source.read) {
            let evidence = Evidence::Inline { path: source.path.clone(), span };
            testimony.value(value, evidence);
        }
    }
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

impl<C> Named for Inline<C> {
    const NAME: Witnessed = Witnessed::Inline;
}

impl Named for Draft {
    const NAME: Witnessed = Witnessed::Draft;
}

impl Named for Heading {
    const NAME: Witnessed = Witnessed::Heading;
}
