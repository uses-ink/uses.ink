//! The witnesses every ladder is built from. Each is written once, generic over the codes and
//! grounds it serves, and says what it testifies to through a small trait the code implements:
//! [`Fronted`] for a frontmatter key, [`Assumed`] for a default, [`Follows`] for a value carried
//! over from another code, [`Listed`] for a pattern's meaning.

mod body;
mod derive;
mod fallback;
mod folder;
mod front;
mod history;
mod name;
mod pattern;

use std::marker::PhantomData;

use model::Evidence;
use model::code::Code;

pub(crate) use body::Body;
pub(crate) use derive::{Derived, Implied};
pub(crate) use fallback::Fallback;
pub(crate) use folder::{Content, Docs, Drafts, Folder, Hidden, Obsidian, Posts, Templates};
pub(crate) use front::{Draft, Front, Heading};
pub(crate) use history::{Changed, Created, History};
pub(crate) use name::{Dated, FileName, Repository, Stem};
pub(crate) use pattern::{Exclude, Include, Pattern};

use crate::interpret::decide::ladder::{Case, Testimony, Witness};

/// What `W` says, only when no witness before it said a value: a last resort that is not a
/// default, so it is never recorded as outranked.
#[derive(Debug, Default)]
pub(crate) struct Otherwise<W>(W);

impl<C: Code, G: ?Sized, W: Witness<C, G>> Witness<C, G> for Otherwise<W> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        if !testimony.is_decided() {
            self.0.testify(case, testimony);
        }
    }
}

/// The author's pin in `ink.toml`, evidence `Config { span }`: first in every ladder.
#[derive(Debug, Default)]
pub(crate) struct Pin<C>(PhantomData<C>);

impl<C: Code, G: ?Sized> Witness<C, G> for Pin<C> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>) {
        if let Some(pin) = &case.pin {
            testimony.value(pin.value.clone(), Evidence::Config { span: pin.span });
        }
    }
}
