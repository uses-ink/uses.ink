//! What a host serves: the site a handle names, the deploy it points at, and that deploy's manifest.

use blob::{Blob, Chunks};
use model::{DeployId, Manifest};

use crate::id::{Id, Site as SiteKind};
use crate::{Control, Error, Handle, Live, Ports, Refusal, SiteState};

/// What a handle serves now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Serving {
    /// The site.
    pub site: Id<SiteKind>,
    /// The pointer it serves from.
    pub live: Live,
}

impl<P: Ports> Control<P> {
    /// The site `handle` names and the deploy it serves, or `None` when no site that takes
    /// changes has the handle or none of its deploys is live. A site being removed serves
    /// nothing.
    pub async fn serving(&self, handle: &Handle) -> Result<Option<Serving>, Error<P>> {
        let Some(site) = self.by_handle(handle).await?.filter(|site| site.state == SiteState::Active) else {
            return Ok(None);
        };
        Ok(self.live(site.id).await?.map(|live| Serving { site: site.id, live }))
    }

    /// The manifest of a deploy of `site`: the blob whose hash is the deploy's id, read back with
    /// every route's blob, media type, status and headers.
    pub async fn manifest(&self, site: Id<SiteKind>, deploy: DeployId) -> Result<Manifest, Error<P>> {
        let tenant = Self::tenant(site)?;
        let stored = self.ports.blob().get(&tenant, deploy.hash()).await?;
        let bytes = stored.ok_or(Refusal::NoManifest { deploy })?.all().await?;
        Manifest::from_canonical(&bytes).map_err(Error::Manifest)
    }
}
