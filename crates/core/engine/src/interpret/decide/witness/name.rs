//! [`FileName`]: what a file's name says, read by a parser `P`: a date prefix ([`Dated`]), a
//! title or slug from the stem ([`Stem`]), a file about the repository ([`Repository`]).

use std::marker::PhantomData;

use model::When;
use model::code::{Code, Date, Ignored, Layout, Slug, Title};

use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};
use crate::interpret::decide::trace::{Named, Reading, Witnessed};
use crate::interpret::pipeline::tree::Scope;

/// Reads a code's value from a file's name.
pub(crate) trait Parse<C: Code> {
    /// The value the name of `file` gives, if it gives one.
    fn parse<G: Filed + ?Sized>(file: &G) -> Option<C::Value>;
}

/// The file's name, read by `P`, evidence `Filename`.
#[derive(Debug, Default)]
pub(crate) struct FileName<P>(PhantomData<P>);

impl<C: Code, P: Parse<C>, G: Filed + ?Sized> Witness<C, G> for FileName<P> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        if let Some(value) = P::parse(case.ground) {
            testimony.value(value, case.ground.named());
        }
    }
}

/// A `YYYY-MM-DD` prefix: `2024-03-01-hello.md` was written on 1 March 2024.
#[derive(Debug, Default)]
pub(crate) struct Dated;

impl Parse<Date> for Dated {
    fn parse<G: Filed + ?Sized>(file: &G) -> Option<Option<When>> {
        file.name().date().map(|day| Some(When::Day(day)))
    }
}

impl Witness<Layout, Scope<'_>> for FileName<Dated> {
    /// Any dated file under the root makes a blog.
    fn testify(&self, case: &Case<'_, Layout, Scope<'_>>, testimony: &mut Testimony<model::Layout>) {
        if let Some(file) = case.ground.inside.iter().find(|file| file.name().date().is_some()) {
            testimony.value(model::Layout::Blog, file.named());
        }
    }
}

/// The stem after its date prefix: `On shelves` and `on-shelves` from `2024-03-01-on_shelves`.
#[derive(Debug, Default)]
pub(crate) struct Stem;

impl Parse<Title> for Stem {
    fn parse<G: Filed + ?Sized>(file: &G) -> Option<String> {
        Some(file.name().title())
    }
}

impl Parse<Slug> for Stem {
    fn parse<G: Filed + ?Sized>(file: &G) -> Option<model::Slug> {
        model::Slug::from_text(file.name().rest()).ok()
    }
}

/// A file about the repository, not the site, at the top of the root (`LICENSE`, `CHANGELOG`),
/// or `ink.toml` at the top of the snapshot. Its whole stem is the name, in any case:
/// `security-checklist.md` is a page.
#[derive(Debug, Default)]
pub(crate) struct Repository;

impl Repository {
    /// Stems of files about the repository.
    const STEMS: &[&str] = &["changelog", "contributing", "code_of_conduct", "security"];

    /// Stems of licence files, which also come one per licence (`LICENSE-MIT`).
    const LICENCES: &[&str] = &["license", "licence"];

    /// The config file, at the top of the snapshot.
    const CONFIG: &str = "ink.toml";

    /// Whether `path`, at the top of its tree, is named as a file about the repository.
    pub(crate) fn names(path: &model::ContentPath) -> bool {
        let stem = path.stem();
        let exact = Self::STEMS.iter().chain(Self::LICENCES).any(|name| stem.eq_ignore_ascii_case(name));
        path.parent().is_none() && (exact || Self::LICENCES.iter().any(|name| Self::is_licence(stem, name)))
    }

    /// Whether `stem` is the licence `name`, then `-` or `.` and an identifier with no lower-case
    /// letter: `LICENSE-MIT`, `licence.APACHE-2.0`, not `license-guide`.
    fn is_licence(stem: &str, name: &str) -> bool {
        let (Some(head), Some(tail)) = (stem.get(..name.len()), stem.get(name.len()..)) else { return false };
        let id = tail.strip_prefix(['-', '.']);
        head.eq_ignore_ascii_case(name) && id.is_some_and(|id| !id.is_empty() && !id.chars().any(char::is_lowercase))
    }

    /// Whether `path` in the snapshot is about the repository rather than any site in it: a
    /// dotfile, or a repository file or README at the top. `site.root` looks past these.
    pub(crate) fn holds(path: &model::ContentPath) -> bool {
        let readme = path.parent().is_none() && path.stem().eq_ignore_ascii_case("readme");
        path.segments().any(|segment| segment.starts_with('.')) || Self::names(path) || readme
    }
}

impl Parse<Ignored> for Repository {
    fn parse<G: Filed + ?Sized>(file: &G) -> Option<bool> {
        (Self::names(file.relative()) || file.path().as_str() == Self::CONFIG).then_some(true)
    }
}

impl Named for FileName<Dated> {
    const NAME: Witnessed = Witnessed::Name(Reading::Dated);
}

impl Named for FileName<Stem> {
    const NAME: Witnessed = Witnessed::Name(Reading::Stem);
}

impl Named for FileName<Repository> {
    const NAME: Witnessed = Witnessed::Name(Reading::Repository);
}
