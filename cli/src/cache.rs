//! Where a build keeps its work for the next: `--cache`, or `<dir>/.ink/cache` beside the
//! default output. The folder is marked, so no command reads it as content, and nothing is
//! written to it until there is something to keep (decision 0040, as measured).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use config::Image;
use host_image::{Pixels, Settings};
use host_native::Disk;
use model::Notice;

/// The cache of one command.
#[derive(Debug)]
pub(crate) struct Caching {
    root: PathBuf,
    disk: Arc<Disk>,
}

impl Caching {
    /// The cache when `--cache` is not given, inside the site's folder.
    pub(crate) const DEFAULT: &'static str = ".ink/cache";

    /// The cache `flag` names, or the default one in `dir`.
    pub(crate) fn new(dir: &Path, flag: Option<&Path>) -> Self {
        let root = flag.map_or_else(|| dir.join(Self::DEFAULT), Path::to_path_buf);
        Self { disk: Arc::new(Disk::open(&root)), root }
    }

    /// The folder it lives in.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// The image work, taking and keeping copies in the cache, under the settings `ink.toml`
    /// gave in `[image]` and the defaults for the keys it left out.
    pub(crate) fn pixels(&self, image: &Image) -> Pixels {
        let mut settings = Settings::default();
        if let Some(widths) = &image.widths {
            settings.widths = widths.value.get().iter().map(|width| u32::from(width.get())).collect();
        }
        if let Some(quality) = &image.avif_quality {
            settings.avif_quality = quality.value.get();
        }
        if let Some(quality) = &image.jpeg_quality {
            settings.jpeg_quality = quality.value.get();
        }
        Pixels::cached(settings, self.disk.clone())
    }

    /// The cache itself, for the pages a render takes from it and keeps in it.
    pub(crate) fn disk(&self) -> &Disk {
        &self.disk
    }

    /// Keeps the cache within its bound, then what it has said.
    pub(crate) fn finish(&self) -> Vec<Notice> {
        self.disk.sweep();
        self.disk.notices()
    }
}
