//! The fetch tests' view of `fixture`'s origin repository: its folders as the host's types.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use std::path::Path;

use fixture::origin::Origin;
use host_native::Fetch;
use model::git::{Depth, Locator, Network, Request};

/// What a test asks of an origin repository, in the host's terms.
pub(crate) trait Fetching: Sized {
    /// The repository written for the test `name`, under cargo's target directory.
    fn named(name: &str) -> Self;
    /// Where the repository is.
    fn locator(&self) -> Locator;
    /// A fetch from it into its cache folder, with only local transports allowed.
    fn fetch(&self) -> Fetch;
    /// A request for it at `depth`.
    fn request(&self, depth: Depth) -> Request;
}

impl Fetching for Origin {
    fn named(name: &str) -> Self {
        Self::new(Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-native-fetch").join(name))
    }

    fn locator(&self) -> Locator {
        self.url().parse().unwrap()
    }

    fn fetch(&self) -> Fetch {
        Fetch { network: Network::Local, ..Fetch::new(&self.cache) }
    }

    fn request(&self, depth: Depth) -> Request {
        let mut request = Request::new(self.locator());
        request.depth = depth;
        request
    }
}
