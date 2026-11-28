//! The plain files published pages use: each raster image as the copies the host made of it, or
//! as it is when the host kept it; every other file from the snapshot's own blobs.

use model::{Diagnostic, MediaType, Problem, Site, Status, Subject};

use crate::Sink;
use crate::error::Error;
use crate::render::Artifact;
use crate::render::output::{Body, Entry, Output, Owner};
use crate::render::picture::{Pictures, Raster};
use crate::render::policy::Cache;

/// Every asset of the site. One of a kind the engine cannot name is `asset.media.unknown`; a
/// raster image the host refused is not served (what refused it was said).
#[derive(Debug)]
pub(crate) struct Assets<'a> {
    pub(crate) pictures: &'a Pictures,
}

impl Artifact for Assets<'_> {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        for asset in site.assets() {
            let Ok(media) = MediaType::try_from(&asset.file) else {
                // a pinned file: no page shows one of these (link resolution refuses it)
                let problem = Problem::AssetMediaUnknown { path: asset.file.clone() };
                out.diagnose(Diagnostic::new(problem).on(Subject::File(asset.file.clone())));
                continue;
            };
            let owner = Owner::Asset(asset.file.clone());
            let made = self.pictures.made(&asset.file);
            if Raster::of(asset).is_some() && made.is_none() {
                continue;
            }
            match made.filter(|made| !made.copies.is_empty()) {
                Some(made) => {
                    for copy in &made.copies {
                        out.serve(Entry {
                            route: copy.route.clone(),
                            media: copy.media,
                            status: Status::Ok,
                            body: Body::Bytes(copy.bytes.clone()),
                            owner: owner.clone(),
                            cache: Cache::Immutable,
                        });
                    }
                }
                None => out.serve(Entry {
                    route: asset.route.clone(),
                    media,
                    status: Status::Ok,
                    body: Body::Blob(asset.blob),
                    owner,
                    cache: Cache::Revalidate,
                }),
            }
        }
        Ok(())
    }
}
