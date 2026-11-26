//! Values carried over from another code: [`Derived`] from a decision already made about the
//! home page, [`Implied`] from what a witness of another code says about the same subject.

use std::marker::PhantomData;

use model::PageKind;
use model::code::{Code, Date, Description, Kind, SiteTitle, Summary, Title};
use model::decision::File;

use crate::interpret::decide::ladder::{Case, Testimony, Witness};
use crate::interpret::site;

/// A code whose value can follow from a value of code `D`.
pub(crate) trait Follows<D: Code>: Code {
    /// The value `value` of `D` gives, if it gives one.
    fn follow(value: &D::Value) -> Option<Self::Value>;
}

impl Follows<Date> for Kind {
    /// A dated page is a post.
    fn follow(value: &Option<model::When>) -> Option<PageKind> {
        value.map(|_| PageKind::Post)
    }
}

impl Follows<Title> for SiteTitle {
    /// The home page's title names the site.
    fn follow(value: &String) -> Option<String> {
        Some(value.clone())
    }
}

impl Follows<Summary> for Description {
    /// The home page's summary, when it has one, describes the site.
    fn follow(value: &Option<String>) -> Option<Option<String>> {
        value.clone().map(Some)
    }
}

/// The home page's decision of `D`, carried over with its evidence.
#[derive(Debug, Default)]
pub(crate) struct Derived<D>(PhantomData<D>);

impl<C: Follows<D>, D: Code<Subject = File>> Witness<C, site::Ground<'_>> for Derived<D> {
    fn testify(&self, case: &Case<'_, C, site::Ground<'_>>, testimony: &mut Testimony<C::Value>) {
        let Some(home) = case.ground.home else { return };
        let Some(decision) = case.ground.journal.decision::<D>(&File(home.path.clone())) else { return };
        if let Some(value) = C::follow(&decision.value) {
            testimony.value(value, decision.evidence.clone());
        }
    }
}

/// What witness `W` says about code `D` on the same subject and ground, carried over: a
/// frontmatter date makes a page a post.
#[derive(Debug, Default)]
pub(crate) struct Implied<D, W> {
    witness: W,
    code: PhantomData<D>,
}

impl<C, D, W, G> Witness<C, G> for Implied<D, W>
where
    C: Follows<D>,
    D: Code<Subject = C::Subject>,
    W: Witness<D, G>,
    G: ?Sized,
{
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        let implying = Case { subject: case.subject, pin: None, patterns: case.patterns, ground: case.ground };
        let mut said = Testimony::default();
        self.witness.testify(&implying, &mut said);
        for (value, evidence) in said.into_values() {
            if let Some(value) = C::follow(&value) {
                testimony.value(value, evidence);
            }
        }
    }
}
