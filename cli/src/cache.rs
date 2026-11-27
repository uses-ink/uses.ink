//! Where a build keeps its work for the next: `--cache`, or `<dir>/.ink/cache` beside the
//! default output. The folder is marked, so no command reads it as content, and nothing is
//! written to it until there is something to keep (decision 0040, as measured).

use std::path::Path;
use std::sync::Arc;

use host_image::{Pixels, Settings};
use host_native::Disk;
use model::Notice;

/// The cache of one command.
#[derive(Debug)]
pub(crate) struct Caching {
    disk: Arc<Disk>,
}

impl Caching {
    /// The cache when `--cache` is not given, inside the site's folder.
    pub(crate) const DEFAULT: &'static str = ".ink/cache";

    /// The cache `flag` names, or the default one in `dir`.
    pub(crate) fn new(dir: &Path, flag: Option<&Path>) -> Self {
        let root = flag.map_or_else(|| dir.join(Self::DEFAULT), Path::to_path_buf);
        Self { disk: Arc::new(Disk::open(root)) }
    }

    /// The image work, taking and keeping copies in the cache.
    pub(crate) fn pixels(&self) -> Pixels {
        Pixels::cached(Settings::default(), self.disk.clone())
    }

    /// Keeps the cache within its bound, then what it has said.
    pub(crate) fn finish(&self) -> Vec<Notice> {
        self.disk.sweep();
        self.disk.notices()
    }
}
