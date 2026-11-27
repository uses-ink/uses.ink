//! `ink:source-git` as a wasm component: the git source's native core (`uses-ink-source-git`)
//! behind the `ink:source` world, reading through `ink:host/git`. Built for
//! `wasm32-unknown-unknown` and wrapped with `wit-component` by `cargo xtask component`
//! (decision 0035). Nothing here decides anything: it converts and calls.

use model::git::{Grant, History, Locator, Revision, Want};
use sdk::Host;
use sdk::bind::ink::host::git as fetched;
use sdk::bind::ink::model::snapshot as wire;
use source_git::{Source, Unread};

mod bind;

use bind::exports::source as served;

/// The source, as the component exports it.
pub(crate) struct GitSource;

impl GitSource {
    /// The want from the WIT; a field the model refuses is no want.
    fn want(want: served::Want) -> Option<Want> {
        let mut made = Want::new(want.locator.parse::<Locator>().ok()?);
        made.revision = want.revision.map(|text| text.parse::<Revision>()).transpose().ok()?;
        made.grant = want.grant.map(|text| text.parse::<Grant>()).transpose().ok()?;
        made.history = if want.history { History::Wanted } else { History::Unneeded };
        Some(made)
    }

    /// Why a read failed, as the WIT says it.
    fn refused(unread: &Unread<sdk::Unfetched>) -> served::Refused {
        match unread {
            Unread::Refused(refused) => {
                fetched::Refusal::try_from(refused).map_or(served::Refused::Malformed, served::Refused::Fetch)
            }
            Unread::Version { fetched, .. } => served::Refused::Commit(fetched.to_string()),
            _ => served::Refused::Malformed,
        }
    }
}

impl served::Guest for GitSource {
    fn read(want: served::Want) -> Result<wire::Snapshot, served::Refused> {
        let want = Self::want(want).ok_or(served::Refused::Malformed)?;
        let snapshot = Source(Host).read(&want).map_err(|unread| Self::refused(&unread))?;
        wire::Snapshot::try_from(&snapshot).map_err(|_| served::Refused::Malformed)
    }
}
