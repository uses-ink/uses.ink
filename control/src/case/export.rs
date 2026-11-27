//! Export: everything a deploy serves, handed to a sink.

use blob::{Blob, Chunks};
use db::Db;
use model::{Bytes, DeployId, Hash, Route};
use sea_query::{Expr, ExprTrait, Query};

use crate::id::{Id, Site as SiteKind};
use crate::listed::Listed;
use crate::{Control, Error, Failed, Ports, Refusal, table};

/// One file of an export.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Exported {
    /// Where the site serves it.
    pub route: Route,
    /// The hash of its bytes.
    pub blob: Hash,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// Where an export goes: a folder, an archive, a socket. It takes one file at a time, so a site
/// is never held whole in memory.
#[expect(async_fn_in_trait, reason = "a sink may write through a Workers binding whose futures are not Send")]
pub trait Sink {
    /// The sink's own error.
    type Error: std::error::Error + 'static;

    /// Takes the next file, in route order.
    async fn file(&mut self, file: Exported) -> Result<(), Self::Error>;
}

/// What an export wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Summary {
    /// The deploy exported.
    pub deploy: DeployId,
    /// How many files were written.
    pub files: u64,
    /// How many bytes they hold.
    pub bytes: Bytes,
}

impl<P: Ports> Control<P> {
    /// Writes every file of a deploy to `sink`, in route order: the live deploy, or `deploy` when
    /// one is named. Every blob is verified against its hash as it is read, so an export is never
    /// quietly wrong.
    pub async fn export<K: Sink>(
        &self,
        site: Id<SiteKind>,
        deploy: Option<DeployId>,
        sink: &mut K,
    ) -> Result<Summary, Failed<P, K::Error>> {
        let site = self.site(site).await?;
        let tenant = Self::tenant(site.id).map_err(Error::from)?;
        let deploy = match deploy {
            Some(deploy) => {
                let found = Query::select()
                    .column(table::Deploy::Id)
                    .from(table::Deploy::Table)
                    .and_where(Expr::col(table::Deploy::SiteId).eq(site.id.to_string()))
                    .and_where(Expr::col(table::Deploy::Id).eq(deploy.to_string()))
                    .to_owned();
                let held = self.ports.db().first(found.into()).await;
                held.map_err(Error::from)?.ok_or(Error::from(Refusal::NoDeploy { site: site.id, deploy }))?;
                deploy
            }
            None => self.live(site.id).await?.ok_or(Error::from(Refusal::NotLive { site: site.id }))?.deploy,
        };
        let blobs = self.ports.blob();
        let manifest = blobs.get(&tenant, deploy.hash()).await.map_err(Error::from)?;
        let manifest = manifest.ok_or(Error::from(Refusal::NoManifest { deploy }))?.all().await.map_err(Error::from)?;
        let listed = Listed::try_from(manifest.as_slice()).map_err(Error::Manifest)?;
        let mut summary = Summary { deploy, files: 0, bytes: Bytes(0) };
        for (route, blob) in listed.entries() {
            let bytes = blobs.get(&tenant, *blob).await.map_err(Error::from)?;
            let bytes = bytes.ok_or(Error::from(Refusal::MissingBlob { route: route.clone(), blob: *blob }))?;
            let bytes = bytes.all().await.map_err(Error::from)?;
            summary.files += 1;
            summary.bytes = Bytes(summary.bytes.count().saturating_add(bytes.len() as u64));
            sink.file(Exported { route: route.clone(), blob: *blob, bytes }).await.map_err(Failed::Other)?;
        }
        Ok(summary)
    }
}
