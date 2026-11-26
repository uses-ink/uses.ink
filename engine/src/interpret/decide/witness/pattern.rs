//! [`Pattern`]: an `include` or `exclude` pattern of `ink.toml` matching the file, evidence
//! `Config`. What a match means is the code's: an `exclude` ignores a file, an `include` keeps
//! it and, in a vault, selects it for publishing.

use std::marker::PhantomData;

use model::code::{Code, Ignored, Publish};
use model::{ContentPath, Evidence, Span};

use crate::Patterns;
use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};

/// One list of patterns.
pub(crate) trait List {
    /// Where the first pattern of the list matching `path` is written.
    fn find(patterns: &dyn Patterns, path: &ContentPath) -> Option<Span>;
}

/// The `include` patterns.
#[derive(Debug, Default)]
pub(crate) struct Include;

impl List for Include {
    fn find(patterns: &dyn Patterns, path: &ContentPath) -> Option<Span> {
        patterns.included(path)
    }
}

/// The `exclude` patterns.
#[derive(Debug, Default)]
pub(crate) struct Exclude;

impl List for Exclude {
    fn find(patterns: &dyn Patterns, path: &ContentPath) -> Option<Span> {
        patterns.excluded(path)
    }
}

/// A yes-or-no code a pattern of list `L` decides.
pub(crate) trait Listed<L>: Code<Value = bool> {
    /// The value a match means.
    const MATCHED: bool;
}

impl Listed<Exclude> for Ignored {
    const MATCHED: bool = true;
}

impl Listed<Include> for Ignored {
    const MATCHED: bool = false;
}

impl Listed<Include> for Publish {
    const MATCHED: bool = true;
}

/// A pattern of list `L` matching the file's path in the snapshot.
#[derive(Debug, Default)]
pub(crate) struct Pattern<L>(PhantomData<L>);

impl<C: Listed<L>, L: List, G: Filed + ?Sized> Witness<C, G> for Pattern<L> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<bool>) {
        if let Some(span) = L::find(case.patterns, case.ground.path()) {
            testimony.value(C::MATCHED, Evidence::Config { span });
        }
    }
}
