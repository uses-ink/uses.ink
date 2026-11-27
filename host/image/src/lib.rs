//! The builder's image work, behind [`engine::Images`]: an image's bytes decoded within a pixel
//! budget (a decompression bomb is refused from its header, before a pixel is decoded), its
//! EXIF orientation applied, its metadata left behind, resized to a width ladder without ever
//! growing, encoded as AVIF and as JPEG (PNG when it has transparency), and its thumbhash drawn
//! as a placeholder. Pure Rust: no C, no assembly.
//!
//! Deterministic: the same bytes under the same [`Settings`] give the same copies on every
//! machine, whatever its core count or SIMD (`encode` says how each step is pinned), so a
//! [`Key`] of the bytes' hash and the settings names the work. [`Pixels`] keeps what it made by
//! key, so an image shown twice, or built again in the same process, is prepared once; given an
//! [`engine::Cache`], it keeps it there too, so a later build takes it instead of encoding it
//! again, and gives the same bytes either way.
//!
//! JPEG, PNG and WebP are decoded; GIF and WebP with more than one frame are kept as they are
//! (a still copy would lose the animation, and no reader page runs script to play one); AVIF
//! is kept as it is, since decoding it takes dav1d, which is C. Colour profiles are not
//! converted: pixels are taken as sRGB.

mod decode;
mod encode;
mod error;
mod jpeg;
mod record;
mod settings;
mod thumbhash;
mod work;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use engine::cache::Cache;
use engine::image::{Prepared, Source};
use engine::{Images, cache};

pub use error::{Reason, Refused};
pub use settings::{Key, Limits, Settings};

use record::Record;

/// The image work, under one set of [`Settings`], keeping what it made by [`Key`].
pub struct Pixels {
    settings: Settings,
    made: Mutex<BTreeMap<Key, Prepared>>,
    /// Where work is kept between builds; `()` keeps nothing.
    cache: Arc<dyn Cache>,
    limits: Limits,
}

impl Pixels {
    /// The work under `settings`, nothing made yet, nothing kept between builds.
    pub fn new(settings: Settings) -> Self {
        Self::cached(settings, Arc::new(()))
    }

    /// The work under `settings`, taking what an earlier build kept in `cache` and keeping what
    /// it makes there.
    pub fn cached(settings: Settings, cache: Arc<dyn Cache>) -> Self {
        Self { settings, made: Mutex::default(), cache, limits: Limits::default() }
    }

    /// Its settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Its limits.
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The same work under `limits`; the bytes made are the same under any threads or opening
    /// limit, and a lower pixel limit refuses what it did not.
    #[must_use]
    pub const fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// What was made under `source`'s key in this process, or kept by an earlier build. What
    /// does not read back as a record is rejected, so the cache drops it and says so.
    fn taken(&self, source: &Source<'_>) -> Option<Prepared> {
        let key = Key::of(source.blob, &self.settings, &self.limits);
        if let Some(made) = self.made.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
            return Some(made.clone());
        }
        let kept = cache::Key::from(key);
        let Some(prepared) = Record::read(self.cache.get(&kept)?) else {
            self.cache.reject(&kept);
            return None;
        };
        self.made.lock().unwrap_or_else(PoisonError::into_inner).insert(key, prepared.clone());
        Some(prepared)
    }

    /// Keeps what was made of `source` for this process and, when it is new, for later builds;
    /// what a record cannot say is kept in this process only.
    fn keep(&self, source: &Source<'_>, prepared: &Prepared) {
        let key = Key::of(source.blob, &self.settings, &self.limits);
        let new = self.made.lock().unwrap_or_else(PoisonError::into_inner).insert(key, prepared.clone()).is_none();
        if let (true, Some(values)) = (new, Record::values(prepared)) {
            let values: Vec<&[u8]> = values.iter().map(Vec::as_slice).collect();
            self.cache.put(&cache::Key::from(key), &values);
        }
    }
}

impl Default for Pixels {
    /// The default settings, nothing kept between builds.
    fn default() -> Self {
        Self::new(Settings::default())
    }
}

impl std::fmt::Debug for Pixels {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let made = self.made.lock().unwrap_or_else(PoisonError::into_inner).len();
        out.debug_struct("Pixels").field("settings", &self.settings).field("made", &made).finish_non_exhaustive()
    }
}

impl Images for Pixels {
    type Refusal = Refused;

    /// Each image opened on one thread and its copies encoded on any, on as many threads as
    /// the machine has cores (at most [`Limits::workers`]), with at most [`Limits::opening`] images
    /// decoded at once; the
    /// order of the work changes no byte.
    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Refused>> {
        let cores = thread::available_parallelism().map_or(1, usize::from);
        let threads = cores.min(self.limits.workers.get());
        work::Run::new(self, sources).finish(threads)
    }
}
