//! The plain files published pages use, served from the snapshot's own blobs.

use model::{ContentPath, Diagnostic, MediaType, Problem, Site, Status, Subject};

use crate::error::Error;
use crate::render::Artifact;
use crate::render::output::{Body, Entry, Output, Owner};

/// Every asset of the site. One of a kind the engine cannot name is `asset.media.unknown`.
#[derive(Debug)]
pub(crate) struct Assets;

impl Assets {
    /// The media type of a plain file, by its extension.
    fn media(path: &ContentPath) -> Option<MediaType> {
        let extension = path.extension()?.to_ascii_lowercase();
        Some(match extension.as_str() {
            "png" => MediaType::Png,
            "jpg" | "jpeg" => MediaType::Jpeg,
            "avif" => MediaType::Avif,
            "webp" => MediaType::Webp,
            "gif" => MediaType::Gif,
            "svg" => MediaType::Svg,
            "ico" => MediaType::Ico,
            "pdf" => MediaType::Pdf,
            _ => return None,
        })
    }
}

impl Artifact for Assets {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        for asset in site.assets() {
            let Some(media) = Self::media(&asset.file) else {
                let problem = Problem::AssetMediaUnknown { path: asset.file.clone() };
                out.diagnose(Diagnostic::new(problem).on(Subject::File(asset.file.clone())));
                continue;
            };
            out.serve(Entry {
                route: asset.route.clone(),
                media,
                status: Status::Ok,
                body: Body::Blob(asset.blob),
                owner: Owner::Asset(asset.file.clone()),
            });
        }
        Ok(())
    }
}
