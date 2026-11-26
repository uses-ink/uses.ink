//! One decision's precedence: rungs in order, the first present one wins, every other present
//! one is recorded as an outranked alternative. The author's pin is always the first rung.

use std::marker::PhantomData;

use model::{Alternative, Decided, Decision, DecisionCode, DefaultReason, Evidence, Rejection, Subject};

use crate::interpret::record::Record;
use crate::pins::Pins;

/// A decision code with the type of its value, so a ladder cannot mix codes.
pub(crate) trait Code {
    type Value: Clone;
    const CODE: DecisionCode;
    fn wrap(value: Self::Value) -> Decided;
    fn unwrap(decided: &Decided) -> Option<&Self::Value>;
}

/// Declares one marker type per decision code, each implementing [`Code`] for its variant of
/// `Decided`. The same impl over a list of types, which a generic cannot write.
///
/// ```text
/// codes! { Title(String); }
/// // makes `code::Title`, whose `Value` is `String` and whose `CODE` is `DecisionCode::Title`
/// ```
macro_rules! codes {
    ($($variant:ident($value:ty);)*) => {
        /// One marker type per decision code.
        pub(crate) mod code {
            $(
                #[derive(Debug)]
                pub(crate) struct $variant;

                impl super::Code for $variant {
                    type Value = $value;
                    const CODE: model::DecisionCode = model::DecisionCode::$variant;

                    fn wrap(value: $value) -> model::Decided {
                        model::Decided::$variant(value)
                    }

                    fn unwrap(decided: &model::Decided) -> Option<&$value> {
                        match decided {
                            model::Decided::$variant(value) => Some(value),
                            _ => None,
                        }
                    }
                }
            )*
        }
    };
}

codes! {
    Root(model::Root);
    Layout(model::Layout);
    Ignored(bool);
    Home(Option<model::ContentPath>);
    Kind(model::PageKind);
    Publish(bool);
    Title(String);
    Date(Option<model::When>);
    Updated(Option<model::When>);
    Slug(model::Slug);
    Tags(Vec<model::Tag>);
    Nav(Vec<model::ContentPath>);
    Summary(Option<String>);
    SiteTitle(String);
    Description(Option<String>);
    Authors(Vec<model::Author>);
    Language(model::Language);
}

/// The candidates of one decision, in precedence order.
#[derive(Debug)]
pub(crate) struct Ladder<C: Code> {
    subject: Subject,
    rungs: Vec<(C::Value, Evidence)>,
    bounds: Vec<(C::Value, Evidence)>,
    code: PhantomData<C>,
}

impl<C: Code> Ladder<C> {
    /// A ladder whose first rung is the author's pin, if `ink.toml` has one.
    pub(crate) fn new(subject: Subject, pins: &impl Pins, record: &mut Record) -> Self {
        let pinned = record.pinned::<C>(pins, &subject);
        let ladder = Self { subject, rungs: Vec::new(), bounds: Vec::new(), code: PhantomData };
        ladder.rung(pinned)
    }

    /// The next candidate, when its input is present.
    #[must_use]
    pub(crate) fn rung(mut self, rung: Option<(C::Value, Evidence)>) -> Self {
        self.rungs.extend(rung);
        self
    }

    /// A value only bounded by a fact (`Known::NotAfter`): never chosen, always recorded as
    /// rejected for being a bound.
    #[must_use]
    pub(crate) fn bound(mut self, bound: Option<(C::Value, Evidence)>) -> Self {
        self.bounds.extend(bound);
        self
    }

    /// Decides, taking `value` with `evidence` when no rung is present.
    pub(crate) fn or(self, value: C::Value, evidence: Evidence, record: &mut Record) -> C::Value {
        self.finish((value, evidence), record)
    }

    /// Decides, taking the default `value` when no rung is present. The reason is
    /// [`DefaultReason::OnlyBound`] when a bound was all there was.
    pub(crate) fn or_default(self, value: C::Value, reason: DefaultReason, record: &mut Record) -> C::Value {
        let reason = if self.rungs.is_empty() && !self.bounds.is_empty() { DefaultReason::OnlyBound } else { reason };
        self.finish((value, Evidence::Default { reason }), record)
    }

    fn finish(self, last: (C::Value, Evidence), record: &mut Record) -> C::Value {
        let mut rungs = self.rungs.into_iter();
        let (value, evidence) = rungs.next().unwrap_or(last);
        let mut decision = Decision::new(C::wrap(value.clone()), self.subject, evidence);
        for (lost, evidence) in rungs {
            decision = decision.rejecting(Alternative::new(C::wrap(lost), evidence, Rejection::Outranked));
        }
        for (bound, evidence) in self.bounds {
            decision = decision.rejecting(Alternative::new(C::wrap(bound), evidence, Rejection::OnlyBound));
        }
        record.decide(decision);
        value
    }
}
