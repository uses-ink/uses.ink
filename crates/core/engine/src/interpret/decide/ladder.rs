//! One mechanism for every decision: an ordered list of witnesses, each able to testify to a
//! value with its evidence. The first value testified wins and is recorded with its evidence
//! alone (decision 0042); a bound a witness offers is never chosen.

use std::fmt;
use std::marker::PhantomData;

use model::AnyDecision;
use model::Evidence;
use model::code::Code;
use model::decision::Decision;

use crate::error::Error;
use crate::host::Pin;
use crate::interpret::decide::Explain;
use crate::interpret::decide::trace::{Heard, Named, Rung, Standing, Trace, Witnessed};
use crate::interpret::journal::Journal;
use crate::{Patterns, Pins};

/// Something that can testify about a decision of `C`, on what ground `G` holds: a file, the
/// snapshot, the site so far. A tuple of witnesses is a witness, testifying in order; so is a
/// list of boxed ones a plugin adds at run time.
pub(crate) trait Witness<C: Code, G: ?Sized> {
    /// Adds what it can say about `case` to `testimony`, or nothing.
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>);
}

/// What a witness looks at: the subject, the author's pin, the `ink.toml` patterns and the
/// ground the stage deciding it holds.
pub(crate) struct Case<'a, C: Code, G: ?Sized> {
    /// What is decided about.
    pub(crate) subject: &'a C::Subject,
    /// The value `ink.toml` pins, if it pins one.
    pub(crate) pin: Option<Pin<C::Value>>,
    /// The `include` and `exclude` patterns.
    pub(crate) patterns: &'a dyn Patterns,
    /// What the deciding stage knows.
    pub(crate) ground: &'a G,
}

impl<C: Code, G: ?Sized> fmt::Debug for Case<'_, C, G> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Case").field("subject", self.subject).field("pin", &self.pin).finish_non_exhaustive()
    }
}

/// Everything witnesses said about one decision, in the order they said it.
#[derive(Debug)]
pub(crate) struct Testimony<V> {
    said: Vec<(V, Evidence, Standing)>,
    failure: Option<Error>,
    /// When traced, each rung's witness and where its testimony starts in `said`.
    rungs: Option<Vec<(Witnessed, usize)>>,
}

impl<V> Default for Testimony<V> {
    fn default() -> Self {
        Self { said: Vec::new(), failure: None, rungs: None }
    }
}

impl<V> Testimony<V> {
    /// A value, with the evidence for it.
    pub(crate) fn value(&mut self, value: V, evidence: Evidence) {
        self.said.push((value, evidence, Standing::Value));
    }

    /// A bound on the value: never chosen.
    pub(crate) fn bound(&mut self, value: V, evidence: Evidence) {
        self.said.push((value, evidence, Standing::Bound));
    }

    /// A witness that could not say what it should have; the decision fails with it if no
    /// value was said.
    pub(crate) fn fail(&mut self, error: Error) {
        self.failure.get_or_insert(error);
    }

    /// Whether a value, not only a bound, was said.
    pub(crate) fn is_decided(&self) -> bool {
        self.said.iter().any(|(_, _, standing)| *standing == Standing::Value)
    }

    /// Whether bounds were said and nothing else.
    pub(crate) fn is_bounded(&self) -> bool {
        !self.said.is_empty() && !self.is_decided()
    }

    /// The next rung of a traced ladder starts: what is said from here on is `witness`'s.
    pub(crate) fn rung(&mut self, witness: Witnessed) {
        if let Some(rungs) = &mut self.rungs {
            rungs.push((witness, self.said.len()));
        }
    }

    /// Every value said, as `C`'s testimony: for a witness that testifies through another.
    pub(crate) fn into_values(self) -> impl Iterator<Item = (V, Evidence)> {
        self.said.into_iter().filter(|(_, _, standing)| *standing == Standing::Value).map(|(v, e, _)| (v, e))
    }
}

impl<V: model::Value> Testimony<V> {
    /// The first value said, with its evidence, as `C`'s decision on `subject`; else the
    /// failure a witness gave, or the engine's bug of nothing said.
    fn decide<C: Code<Value = V>>(self, subject: &C::Subject) -> Result<Decision<C>, Error> {
        let chosen = self.said.into_iter().find(|(_, _, standing)| *standing == Standing::Value);
        let Some((value, evidence, _)) = chosen else {
            return Err(self.failure.unwrap_or(Error::Missing { code: C::CODE, subject: subject.clone().into() }));
        };
        Ok(Decision::new(subject.clone(), value, evidence))
    }

    /// What each rung said, as `C`'s decisions on `subject`; no rungs when not traced.
    fn trace<C: Code<Value = V>>(&self, subject: &C::Subject) -> Trace {
        let chosen = self.said.iter().position(|(_, _, standing)| *standing == Standing::Value);
        let rungs = self.rungs.as_deref().unwrap_or_default();
        let ends = rungs.iter().skip(1).map(|(_, start)| *start).chain(std::iter::once(self.said.len()));
        let rungs = rungs
            .iter()
            .zip(ends)
            .map(|(&(witness, start), end)| Rung {
                witness,
                heard: (start..end)
                    .filter_map(|at| self.said.get(at).map(|said| (at, said)))
                    .map(|(at, (value, evidence, standing))| Heard {
                        testimony: AnyDecision::from(Decision::<C>::new(
                            subject.clone(),
                            value.clone(),
                            evidence.clone(),
                        )),
                        standing: *standing,
                        chosen: Some(at) == chosen,
                    })
                    .collect(),
            })
            .collect();
        Trace { code: C::CODE, subject: subject.clone().into(), rungs }
    }
}

/// Whether a ladder keeps what every rung said: only for the one decision `ink explain --why`
/// asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tracing {
    /// Only the decision is kept.
    Off,
    /// Every rung is kept.
    On,
}

/// The precedence of one decision code: `W` is its witnesses, usually a tuple, first first.
pub(crate) struct Ladder<C, W> {
    witnesses: W,
    code: PhantomData<C>,
}

impl<C, W: Default> Default for Ladder<C, W> {
    fn default() -> Self {
        Self { witnesses: W::default(), code: PhantomData }
    }
}

impl<C, W: fmt::Debug> fmt::Debug for Ladder<C, W> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_tuple("Ladder").field(&self.witnesses).finish()
    }
}

impl<C: Code, W> Ladder<C, W> {
    /// Hears every witness and decides: the first value wins, with its evidence; what the
    /// others said is dropped. Nothing said is the engine's bug.
    pub(crate) fn decide<G: ?Sized>(&self, case: &Case<'_, C, G>) -> Result<Decision<C>, Error>
    where
        W: Witness<C, G>,
    {
        let mut testimony = Testimony::default();
        self.witnesses.testify(case, &mut testimony);
        testimony.decide::<C>(case.subject)
    }

    /// Decides, keeping what every rung said when `tracing` asks for it.
    pub(crate) fn hear<G: ?Sized>(
        &self,
        case: &Case<'_, C, G>,
        tracing: Tracing,
    ) -> (Result<Decision<C>, Error>, Option<Trace>)
    where
        W: Witness<C, G>,
    {
        match tracing {
            Tracing::Off => (self.decide(case), None),
            Tracing::On => {
                let (decided, trace) = self.trace(case);
                (decided, Some(trace))
            }
        }
    }

    /// Decides as [`Ladder::decide`] does, and keeps what every rung said.
    pub(crate) fn trace<G: ?Sized>(&self, case: &Case<'_, C, G>) -> (Result<Decision<C>, Error>, Trace)
    where
        W: Witness<C, G>,
    {
        let mut testimony = Testimony { rungs: Some(Vec::new()), ..Testimony::default() };
        self.witnesses.testify(case, &mut testimony);
        let trace = testimony.trace::<C>(case.subject);
        (testimony.decide::<C>(case.subject), trace)
    }
}

/// Implements [`Witness`] for a tuple of named witnesses, each testifying in order on a rung of
/// its own. The same impl
/// over tuples of every length up to eight, which a generic cannot write.
///
/// ```text
/// witnesses!(A 0, B 1);
/// // makes `impl<C, G, A: Witness<C, G>, B: Witness<C, G>> Witness<C, G> for (A, B)`
/// ```
macro_rules! witnesses {
    ($($witness:ident $at:tt),+) => {
        impl<C: Code, G: ?Sized, $($witness: Witness<C, G> + Named),+> Witness<C, G> for ($($witness,)+) {
            fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
                $(
                    testimony.rung($witness::NAME);
                    self.$at.testify(case, testimony);
                )+
            }
        }
    };
}

witnesses!(A 0);
witnesses!(A 0, B 1);
witnesses!(A 0, B 1, D 2);
witnesses!(A 0, B 1, D 2, E 3);
witnesses!(A 0, B 1, D 2, E 3, F 4);
witnesses!(A 0, B 1, D 2, E 3, F 4, H 5);
witnesses!(A 0, B 1, D 2, E 3, F 4, H 5, I 6);
witnesses!(A 0, B 1, D 2, E 3, F 4, H 5, I 6, J 7);

/// What a stage decides on: a subject, and the ground it holds about it. One per subject, for
/// every code decided on it.
#[derive(Debug)]
pub(crate) struct About<'a, S, G: ?Sized> {
    /// What is decided about.
    pub(crate) subject: &'a S,
    /// What the deciding stage knows.
    pub(crate) ground: &'a G,
}

impl<S, G: ?Sized> Clone for About<'_, S, G> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, G: ?Sized> Copy for About<'_, S, G> {}

impl<C: Explain, W> Ladder<C, W> {
    /// Decides `C` on `about` from what its ground holds and what `pins` pin, and records the
    /// decision in `journal`. Gives back the value decided.
    pub(crate) fn record<G: ?Sized, P: Pins>(
        &self,
        about: About<'_, C::Subject, G>,
        pins: &P,
        journal: &mut Journal,
    ) -> Option<C::Value>
    where
        W: Witness<C, G>,
    {
        let About { subject, ground } = about;
        let pin = pins.pin::<C>(subject);
        let case = Case { subject, pin, patterns: pins, ground };
        let (decided, trace) = self.hear(&case, journal.tracing(C::CODE, subject));
        journal.traced(trace);
        journal.record(decided)
    }
}

#[cfg(test)]
mod test {
    use model::code::Date;
    use model::decision::File;
    use model::{ContentPath, Provenance, When};

    use super::*;
    use crate::NoPins;

    /// Testifies one value, or one bound, of `page.date`.
    #[derive(Debug)]
    struct Says(Option<When>, Standing);

    impl Witness<Date, ()> for Says {
        fn testify(&self, _case: &Case<'_, Date, ()>, testimony: &mut Testimony<Option<When>>) {
            let evidence = Evidence::History { from: Provenance::Filesystem };
            match self.1 {
                Standing::Value => testimony.value(self.0, evidence),
                Standing::Bound => testimony.bound(self.0, evidence),
            }
        }
    }

    impl Named for Says {
        const NAME: Witnessed = Witnessed::History(crate::interpret::decide::trace::Fact::Created);
    }

    #[test]
    fn the_first_value_wins_past_a_bound() {
        let day = |d| Some(When::Day(jiff::civil::date(2024, 1, d)));
        let ladder: Ladder<Date, (Says, Says, Says)> = Ladder {
            witnesses: (Says(day(2), Standing::Bound), Says(day(1), Standing::Value), Says(day(3), Standing::Value)),
            code: PhantomData,
        };
        let subject = File(ContentPath::parse("a.md").unwrap());
        let case = Case { subject: &subject, pin: None, patterns: &NoPins, ground: &() };
        let decision = ladder.decide(&case).unwrap();
        assert_eq!(decision.value, day(1));
        let (traced, trace) = ladder.trace(&case);
        assert_eq!(traced.unwrap(), decision);
        let heard: Vec<_> = trace
            .rungs
            .iter()
            .map(|rung| rung.heard.iter().map(|h| (h.standing, h.chosen)).collect::<Vec<_>>())
            .collect();
        assert_eq!(heard, [[(Standing::Bound, false)], [(Standing::Value, true)], [(Standing::Value, false)]]);
        assert_eq!(trace.chosen().map(|(at, _)| at), Some(2));
    }
}
