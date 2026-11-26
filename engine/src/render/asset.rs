//! The plain files published pages use, served from the snapshot's own blobs.

use model::{Diagnostic, MediaType, Problem, Site, Status, Subject};

use crate::Record;
use crate::error::Error;
use crate::render::Artifact;
use crate::render::output::{Body, Entry, Output, Owner};

/// Every asset of the site. One of a kind the engine cannot name is `asset.media.unknown`.
#[derive(Debug)]
pub(crate) struct Assets;

impl Artifact for Assets {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        for asset in site.assets() {
            let Ok(media) = MediaType::try_from(&asset.file) else {
                // a pinned file: no page shows one of these (link resolution refuses it)
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
