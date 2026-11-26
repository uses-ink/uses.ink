//! [`History`]: a fact from the source's history about a file, counted only when exact.

use std::marker::PhantomData;

use model::code::Code;
use model::{Evidence, FileFacts, Known, Timestamp, When};

use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};

/// One fact a history knows about a file.
pub(crate) trait Fact {
    /// The fact among `facts`.
    fn known(facts: &FileFacts) -> &Known<Timestamp>;
}

/// When the file was created.
#[derive(Debug, Default)]
pub(crate) struct Created;

impl Fact for Created {
    fn known(facts: &FileFacts) -> &Known<Timestamp> {
        &facts.created
    }
}

/// When the file last changed.
#[derive(Debug, Default)]
pub(crate) struct Changed;

impl Fact for Changed {
    fn known(facts: &FileFacts) -> &Known<Timestamp> {
        &facts.updated
    }
}

/// The history's fact `F`, evidence `History`: a value when exact, a bound when the history only
/// knows it is not after some instant (`Known::NotAfter`), nothing when unknown.
#[derive(Debug, Default)]
pub(crate) struct History<F>(PhantomData<F>);

impl<C: Code<Value = Option<When>>, F: Fact, G: Filed + ?Sized> Witness<C, G> for History<F> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<Option<When>>) {
        let Some(facts) = case.ground.facts() else { return };
        match F::known(facts) {
            Known::Exact { value, from } => {
                testimony.value(Some(When::Instant(*value)), Evidence::History { from: from.clone() });
            }
            Known::NotAfter { value, from } => {
                testimony.bound(Some(When::Instant(*value)), Evidence::History { from: from.clone() });
            }
            _ => {}
        }
    }
}
