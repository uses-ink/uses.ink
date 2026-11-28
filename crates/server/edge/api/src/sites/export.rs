//! Export: the files of a deploy as a tar archive.

use std::future::Future;

use authority_dns::Resolver;
use clock::{Clock, Draw};
use control::{Exported, Id, Ports, Sink, SiteKind};
use model::DeployId;

use crate::Api;
use crate::archive::{self, Writer};
use crate::auth::credential::{Credentials, Principal};
use crate::fault::Fault;
use crate::reply::{Media, Reply, Status};

/// Which deploy of which site to export: the live one when none is named.
#[derive(Clone, Copy, Debug)]
pub struct Route {
    /// The site.
    pub site: Id<SiteKind>,
    /// The deploy, if it is not the live one.
    pub deploy: Option<DeployId>,
}

/// The sink an export writes into: a tar archive in memory, bounded by the export limit. A file
/// is named by its route without the leading slash, and the root route is `index`.
#[derive(Debug)]
pub struct Exports {
    writer: Writer,
}

impl Exports {
    /// A sink that takes at most `limit` bytes.
    pub const fn new(limit: u64) -> Self {
        Self { writer: Writer::new(limit) }
    }

    /// The archive.
    pub fn finish(self) -> Vec<u8> {
        self.writer.finish()
    }
}

impl Sink for Exports {
    type Error = archive::Error;

    fn file(&mut self, file: Exported) -> impl Future<Output = Result<(), archive::Error>> {
        let name = file.route.as_str().trim_matches('/');
        std::future::ready(self.writer.file(if name.is_empty() { "index" } else { name }, &file.bytes))
    }
}

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw, R: Resolver> Api<P, C, K, D, R> {
    pub(crate) async fn export(&self, principal: Principal, route: Route) -> Result<Reply, Fault<P>> {
        self.owned(principal, route.site).await?;
        let mut sink = Exports::new(self.limits.export);
        self.control.export(route.site, route.deploy, &mut sink).await.map_err(Fault::Export)?;
        Ok(Reply::new(Status::Ok, Media::Tar, sink.finish()))
    }
}
