use model::Snapshot;
use model::git::{Cloned, Git, Refused, Request};

use super::{Failed, Fetch};
use crate::blob::Store;
use crate::read::Read;

/// The capability as a native trait: fetching through a [`Fetch`], and storing each snapshot's
/// files in a store the caller keeps. The files of a snapshot are read once, by the last
/// [`Git::snapshot`] call.
#[derive(Debug)]
pub struct Cloning<'a, S> {
    fetch: &'a Fetch,
    store: &'a mut S,
    read: Option<Read>,
}

impl<'a, S: Store> Cloning<'a, S> {
    /// Fetches through `fetch`, storing files in `store`.
    pub fn new(fetch: &'a Fetch, store: &'a mut S) -> Self {
        Self { fetch, store, read: None }
    }

    /// What the last snapshot's read left out and found worth saying: folders skipped, LFS pointers.
    pub fn take(&mut self) -> Option<Read> {
        self.read.take()
    }
}

impl<S: Store> Git for Cloning<'_, S> {
    type Error = Failed;

    fn fetch(&mut self, request: &Request) -> Result<Cloned, Refused<Failed>> {
        self.fetch.run(request)
    }

    fn snapshot(&mut self, cloned: &Cloned) -> Result<Snapshot, Refused<Failed>> {
        let read = self.fetch.read(cloned, self.store)?;
        let snapshot = read.snapshot.clone();
        self.read = Some(read);
        Ok(snapshot)
    }
}
