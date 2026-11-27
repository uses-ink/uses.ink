//! Where a git source is fetched from.

use std::error::Error;

use build::Request;
use host_native::{Memory, Read};
use model::{Diagnostic, Problem};

/// The one thing a builder does not hold itself: reading a repository that is not on its disk.
///
/// The builder is free of any plugin host; whoever runs it says how a remote is fetched (the
/// `ink` binary runs the `ink:source-git` component, decision 0067).
#[expect(async_fn_in_trait, reason = "the build port promises no Send futures either")]
pub trait Remote {
    /// What a failure of the fetch itself is, as opposed to a repository that cannot be read.
    type Error: Error + 'static;

    /// Whether this remote fetches at all. A builder over one that does not says
    /// [`build::Error::Unsupported`] for a git source.
    fn reaches(&self) -> bool;

    /// Reads the repository `request`'s source names into `store`. A repository that cannot be
    /// read is the diagnostic that says why, and the build is refused.
    async fn fetch(&self, request: &Request, store: &mut Memory) -> Result<Result<Read, Box<Diagnostic>>, Self::Error>;
}

/// No remote: a builder that reads only its own disk.
#[derive(Clone, Copy, Debug, Default)]
pub struct Offline;

impl Remote for Offline {
    type Error = std::convert::Infallible;

    fn reaches(&self) -> bool {
        false
    }

    fn fetch(
        &self,
        _: &Request,
        _: &mut Memory,
    ) -> impl Future<Output = Result<Result<Read, Box<Diagnostic>>, Self::Error>> {
        let reason = "this builder holds no remote".to_owned();
        std::future::ready(Ok(Err(Box::new(Diagnostic::new(Problem::SourceRepositoryUnreadable { reason })))))
    }
}
