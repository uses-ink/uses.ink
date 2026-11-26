//! The stage deciding `site.layout`, and from it whether pages are published until said
//! otherwise.

use model::decision::Site;
use model::{Ability, Layout as Arranged};

use crate::Pins;
use crate::interpret::decide::ladder::About;
use crate::interpret::decide::precedence;
use crate::interpret::journal::Journal;
use crate::interpret::page::Selection;
use crate::interpret::stage::Stage;
use crate::interpret::tree::Scope;

/// Decides how the site's files are arranged.
#[derive(Debug)]
pub(crate) struct Layout<'a, P> {
    pub(crate) pins: &'a P,
}

impl<P: Pins> Stage<&Scope<'_>> for Layout<'_, P> {
    type Out = Selection;

    /// Vaults and private sources publish nothing until selected (decision 0017).
    fn run(&self, scope: &Scope<'_>, journal: &mut Journal) -> Selection {
        let layout = precedence::Layout::default().record(About { subject: &Site, ground: scope }, self.pins, journal);
        let private = scope.snapshot.observed().has.contains(&Ability::Private);
        if layout == Some(Arranged::Vault) || private { Selection::Explicit } else { Selection::Implicit }
    }
}
