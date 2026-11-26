//! The builder's image work, behind [`engine::Images`]: an image's bytes decoded within a pixel
//! budget (a decompression bomb is refused from its header, before a pixel is decoded), its
//! EXIF orientation applied, its metadata left behind, resized to a width ladder without ever
//! growing, encoded as AVIF and as JPEG (PNG when it has transparency), and its thumbhash drawn
//! as a placeholder. Pure Rust: no C, no assembly.
//!
//! Deterministic: the same bytes under the same [`Settings`] give the same copies on every
//! machine, whatever its core count or SIMD (`encode` says how each step is pinned), so a
//! [`Key`] of the bytes' hash and the settings names the work. [`Pixels`] keeps what it made by
//! key, so an image shown twice, or built again in the same process, is prepared once.
//!
//! JPEG, PNG and WebP are decoded; GIF and WebP with more than one frame are kept as they are
//! (a still copy would lose the animation, and no reader page runs script to play one); AVIF
//! is kept as it is, since decoding it takes dav1d, which is C. Colour profiles are not
//! converted: pixels are taken as sRGB.

mod decode;
mod encode;
mod error;
mod settings;
mod thumbhash;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;

use engine::Images;
use engine::image::{Prepared, Source, Variants};

pub use error::{Reason, Refused};
pub use settings::{Key, Settings};

use decode::Decoded;

/// The image work, under one set of [`Settings`], keeping what it made by [`Key`].
#[derive(Debug, Default)]
pub struct Pixels {
    settings: Settings,
    made: Mutex<BTreeMap<Key, Prepared>>,
}

impl Pixels {
    /// The work under `settings`, nothing made yet.
    pub fn new(settings: Settings) -> Self {
        Self { settings, made: Mutex::default() }
    }

    /// Its settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// One image prepared, or taken from what was made under the same key.
    fn one(&self, source: &Source<'_>) -> Result<Prepared, Refused> {
        let key = Key::of(source.blob, &self.settings);
        if let Some(made) = self.made.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
            return Ok(made.clone());
        }
        let prepared = self.make(source)?;
        self.made.lock().unwrap_or_else(PoisonError::into_inner).insert(key, prepared.clone());
        Ok(prepared)
    }

    /// One image decoded, copied at each width of the ladder, and its placeholder drawn.
    fn make(&self, source: &Source<'_>) -> Result<Prepared, Refused> {
        let pixels = match Decoded::read(source, &self.settings)? {
            Decoded::Kept(dimensions, reason) => return Ok(Prepared::Kept { dimensions, reason }),
            Decoded::Pixels(pixels) => pixels,
        };
        let refuse = |reason| Refused::new(source.file, reason);
        let mut copies = Vec::new();
        for width in self.settings.ladder(pixels.width) {
            let resized = pixels.resize(width).map_err(refuse)?;
            copies.push(resized.avif(&self.settings).map_err(refuse)?);
            copies.push(resized.fallback(&self.settings).map_err(refuse)?);
        }
        let placeholder = pixels.placeholder().map_err(refuse)?;
        Ok(Prepared::Varied(Variants::new(pixels.dimensions(), copies, placeholder)))
    }
}

impl Images for Pixels {
    type Refusal = Refused;

    /// Each image on a thread of its own, as many at once as the machine has cores (at most
    /// eight, so the decoded images fit in memory); the order of the work changes no byte.
    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Refused>> {
        let workers = thread::available_parallelism().map_or(1, usize::from).clamp(1, 8).min(sources.len());
        let next = AtomicUsize::new(0);
        let done: Mutex<Vec<Option<Result<Prepared, Refused>>>> = Mutex::new(sources.iter().map(|_| None).collect());
        thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(source) = sources.get(index) else { break };
                        let result = self.one(source);
                        let mut done = done.lock().unwrap_or_else(PoisonError::into_inner);
                        if let Some(slot) = done.get_mut(index) {
                            *slot = Some(result);
                        }
                    }
                });
            }
        });
        done.into_inner().unwrap_or_else(PoisonError::into_inner).into_iter().flatten().collect()
    }
}
