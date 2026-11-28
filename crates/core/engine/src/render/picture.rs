//! The site's raster images, prepared by the host before any page is rendered, so a theme knows
//! each one's size, copies and placeholder. Every copy is served at a route holding the hash of
//! its bytes (`/posts/harbour.960.3f2a9c1e5b7d0a4f.avif`), so it can be cached for good; the
//! original is not served, since its metadata (a camera's GPS position among it) is what the
//! copies leave out.

use std::collections::BTreeMap;
use std::fmt;

use model::{Announcement, Asset, ContentPath, Diagnostic, Hash, MediaType, Notice, Picture, Problem, Route};
use model::{Site, Subject, Variant};

use crate::error::Error;
use crate::image::{Encoded, Images, Kept, Prepared, Source};
use crate::render::output::Output;
use crate::{Blobs, Sink};

/// The host's image work over the snapshot's bytes.
#[derive(Debug)]
pub(crate) struct Prepare<'a, B, I> {
    pub(crate) blobs: &'a B,
    pub(crate) images: &'a I,
}

/// One file the engine serves for a prepared image.
#[derive(Debug)]
pub(crate) struct Copied {
    pub(crate) route: Route,
    pub(crate) media: MediaType,
    pub(crate) bytes: Vec<u8>,
}

/// What became of one image: what a theme learns of it, and the files behind that.
#[derive(Debug)]
pub(crate) struct Made {
    picture: Picture,
    /// Empty when the original is served as it is.
    pub(crate) copies: Vec<Copied>,
}

/// Every image prepared, by file. An image missing here was refused, and is not served.
#[derive(Debug, Default)]
pub(crate) struct Pictures(BTreeMap<ContentPath, Made>);

impl Pictures {
    /// What became of the image `file`, if it was prepared.
    pub(crate) fn made(&self, file: &ContentPath) -> Option<&Made> {
        self.0.get(file)
    }

    /// The picture a theme is shown for `asset`.
    pub(crate) fn picture(&self, asset: &Asset) -> Option<Picture> {
        self.made(&asset.file).map(|made| made.picture.clone())
    }
}

/// Whether `asset` is an image the host prepares.
pub(crate) struct Raster;

impl Raster {
    /// Its kind, when it is one.
    pub(crate) fn of(asset: &Asset) -> Option<MediaType> {
        MediaType::try_from(&asset.file).ok().filter(|media| Source::RASTER.contains(media))
    }
}

impl<B: Blobs, I: Images> Prepare<'_, B, I> {
    /// Asks the host for every raster image of `site` at once, and says what it refused or kept.
    pub(crate) fn run(&self, site: &Site, out: &mut Output<'_>) -> Pictures {
        let _stage = logging::stage!(logging::Stage::Image, "prepare").entered();
        let mut read = Vec::new();
        for asset in site.assets() {
            let Some(media) = Raster::of(asset) else { continue };
            if let Ok(bytes) = self.blobs.read(&asset.blob) {
                read.push((asset, media, bytes));
            } else {
                let problem = Problem::SourceBlobMissing { path: asset.file.clone() };
                out.diagnose(Diagnostic::new(problem).on(Subject::File(asset.file.clone())));
            }
        }
        let sources: Vec<Source<'_>> =
            read.iter().map(|(asset, media, bytes)| Source::new(&asset.file, *media, asset.blob, bytes)).collect();
        let prepared = self.images.prepare(&sources);
        if prepared.len() != sources.len() {
            out.fail(Error::Images { asked: sources.len(), count: prepared.len() });
        }
        let mut pictures = Pictures::default();
        for ((asset, _, _), result) in read.iter().zip(prepared) {
            let subject = Subject::File(asset.file.clone());
            let made = match result {
                Ok(prepared) => Self::made(asset, prepared, out),
                Err(refusal) => {
                    out.diagnose(refusal.into().on(subject));
                    continue;
                }
            };
            match made {
                Ok(made) => {
                    pictures.0.insert(asset.file.clone(), made);
                }
                Err(error) => out.fail(error),
            }
        }
        pictures
    }

    /// The picture and files of what the host made of `asset`.
    fn made(asset: &Asset, prepared: Prepared, out: &mut Output<'_>) -> Result<Made, Error> {
        let variants = match prepared {
            Prepared::Kept { dimensions, reason } => {
                let path = asset.file.clone();
                let announcement = match reason {
                    Kept::Animated => Announcement::ImageAnimationKept { path: path.clone() },
                    _ => Announcement::ImageFormatKept { path: path.clone() },
                };
                out.notice(Notice::new(announcement).on(Subject::File(path)));
                return Ok(Made { picture: Picture::new(dimensions, Vec::new(), None), copies: Vec::new() });
            }
            Prepared::Varied(variants) => variants,
        };
        let mut copies = Vec::new();
        let mut shown = Vec::new();
        for encoded in variants.copies {
            let copy = Name::Width(encoded.dimensions.width).copy(&asset.route, encoded.media, encoded.bytes)?;
            shown.push(Variant::new(copy.media, encoded.dimensions, copy.route.clone()));
            copies.push(copy);
        }
        let placeholder = match variants.placeholder {
            Some(Encoded { media, bytes, .. }) => {
                let copy = Name::Placeholder.copy(&asset.route, media, bytes)?;
                let route = copy.route.clone();
                copies.push(copy);
                Some(route)
            }
            None => None,
        };
        Ok(Made { picture: Picture::new(variants.dimensions, shown, placeholder), copies })
    }
}

/// What a copy's file name says it is, between the original's name and the hash of its bytes.
#[derive(Clone, Copy, Debug)]
enum Name {
    /// A copy this many pixels wide.
    Width(u32),
    /// The placeholder.
    Placeholder,
}

impl Name {
    /// The copy of `bytes` beside the original at `original`: `/posts/harbour.jpg` gives
    /// `/posts/harbour.960.<hash>.avif`, the hash cut to 16 hex digits, the extension the media
    /// type's subtype.
    fn copy(self, original: &Route, media: MediaType, bytes: Vec<u8>) -> Result<Copied, Error> {
        let named = Named { original, name: self, hash: Hash::of(&bytes), media };
        let written = named.to_string();
        let route = written.parse().map_err(|source| Error::Variant { written, source })?;
        Ok(Copied { route, media, bytes })
    }
}

/// A copy's route, as text.
#[derive(Debug)]
struct Named<'a> {
    original: &'a Route,
    name: Name,
    hash: Hash,
    media: MediaType,
}

impl Named<'_> {
    /// How many hex digits of the hash a route keeps: 64 bits, so two copies of a site never
    /// share one by chance.
    const DIGITS: usize = 16;
}

impl fmt::Display for Named<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let original = self.original.as_str();
        let stem = original.rsplit_once('.').map_or(original, |(stem, _)| stem);
        let hex = self.hash.to_string();
        let hash = hex.get(..Self::DIGITS).unwrap_or(&hex);
        let essence = self.media.essence();
        let extension = essence.rsplit_once('/').map_or(essence, |(_, subtype)| subtype);
        match self.name {
            Name::Width(width) => write!(out, "{stem}.{width}.{hash}.{extension}"),
            Name::Placeholder => write!(out, "{stem}.placeholder.{hash}.{extension}"),
        }
    }
}
