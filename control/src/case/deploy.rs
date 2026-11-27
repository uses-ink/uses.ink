//! Deploys, the live pointer, rollback and resume.

use blob::Blob;
use db::{Db, Statement};
use jiff::Timestamp;
use model::{Bytes, DeployId};

use crate::deploy::{self, Deploy, Live, Published, Recorded, Resumed};
use crate::id::{Id, Site as SiteKind};
use crate::{Control, Error, Event, Ports, Refusal};

impl<P: Ports> Control<P> {
    /// Records the deploy a build made and, unless publishing is paused, makes it live.
    ///
    /// Every blob the manifest names must be in the site's tenant already, and the manifest is
    /// stored as a blob too: its hash is the deploy's id, so the same site built twice is the
    /// same deploy and recording it again changes nothing. The pointer moves in one statement
    /// that records the previous deploy, so two builds finishing at once cannot lose the
    /// rollback target.
    pub async fn deploy(&self, new: deploy::New) -> Result<Recorded, Error<P>> {
        const DEPLOY: &str = "INSERT INTO deploy (site_id, id, created_at, job_id, files, bytes) VALUES (?, ?, ?, ?, ?, ?) \
            ON CONFLICT (site_id, id) DO NOTHING";
        const POINT: &str = "INSERT INTO site_live (site_id, deploy_id, previous_id, paused, set_at) VALUES (?, ?, NULL, 0, ?) \
            ON CONFLICT (site_id) DO UPDATE SET previous_id = site_live.deploy_id, deploy_id = excluded.deploy_id, \
            set_at = excluded.set_at WHERE site_live.paused = 0 AND site_live.deploy_id != excluded.deploy_id";
        const HISTORY: &str = "INSERT INTO publication (site_id, deploy_id, previous_id, cause, at) \
            SELECT site_id, deploy_id, previous_id, 'publish', set_at FROM site_live \
            WHERE site_id = ? AND deploy_id = ? AND set_at = ? AND paused = 0";
        let site = self.active(new.site).await?;
        let tenant = Self::tenant(site.id)?;
        let blobs = self.ports.blob();
        let mut bytes = Bytes(0);
        for entry in new.manifest.entries() {
            let size = blobs.stat(&tenant, entry.blob()).await?;
            let size = size.ok_or_else(|| Refusal::MissingBlob { route: entry.route().clone(), blob: entry.blob() })?;
            bytes = Bytes(bytes.count().saturating_add(size.count()));
        }
        let id = new.manifest.id();
        blobs.put(&tenant, id.hash(), [new.manifest.canonical()]).await?;
        let (site_id, deploy_id, at) = (site.id.to_string(), id.to_string(), new.at.as_millisecond());
        let before = self.live(site.id).await?;
        let batch = vec![
            Event::DeployAdded { deploy: id }.fresh(site.id, new.at, id).map_err(Error::Log)?,
            Statement::prepare(DEPLOY)
                .bind(site_id.clone())
                .bind(deploy_id.clone())
                .bind(at)
                .bind(new.build.map(|build| build.to_string()))
                .bind((new.manifest.entries().len() as u64).cast_signed())
                .bind(bytes.count().cast_signed()),
            Statement::prepare(POINT).bind(site_id.clone()).bind(deploy_id.clone()).bind(at),
            Statement::prepare(HISTORY).bind(site_id).bind(deploy_id).bind(at),
            Event::DeployPublished { deploy: id, previous: before.map(|live| live.deploy) }
                .moved(site.id, new.at, id)
                .map_err(Error::Log)?,
        ];
        let outcomes = self.ports.db().batch(batch).await?;
        let moved = outcomes.get(2).is_some_and(|outcome| outcome.changes == 1);
        let published = match (moved, before) {
            (true, _) => Published::Live { previous: before.map(|live| live.deploy) },
            (false, Some(live)) if live.paused => Published::Held,
            (false, _) => Published::Unchanged,
        };
        Ok(Recorded { deploy: id, published })
    }

    /// The pointer `site` serves from, if a deploy is live.
    pub async fn live(&self, site: Id<SiteKind>) -> Result<Option<Live>, Error<P>> {
        const SELECT: &str = "SELECT deploy_id, previous_id, paused, set_at FROM site_live WHERE site_id = ?";
        match self.ports.db().first(Statement::prepare(SELECT).bind(site.to_string())).await? {
            Some(row) => Ok(Some(Live::try_from(&row)?)),
            None => Ok(None),
        }
    }

    /// The most recent `limit` deploys of `site`, newest first, each saying whether it is live.
    pub async fn deploys(&self, site: Id<SiteKind>, limit: u32) -> Result<Vec<Deploy>, Error<P>> {
        const SELECT: &str = "SELECT d.id, d.created_at, d.job_id, d.files, d.bytes, COALESCE(l.deploy_id = d.id, 0) AS live \
            FROM deploy d LEFT JOIN site_live l ON l.site_id = d.site_id \
            WHERE d.site_id = ? ORDER BY d.created_at DESC, d.rowid DESC LIMIT ?";
        self.site(site).await?;
        let rows = self.ports.db().rows(Statement::prepare(SELECT).bind(site.to_string()).bind(limit)).await?;
        Ok(rows.iter().map(Deploy::try_from).collect::<Result<_, _>>()?)
    }

    /// Makes an older deploy live again and pauses automatic publishing until [`Control::resume`]
    /// (architecture section 11): a build finishing later is recorded, and does not undo the
    /// rollback. The deploy replaced becomes the previous one, so rolling back twice returns.
    pub async fn rollback(&self, site: Id<SiteKind>, to: DeployId, at: Timestamp) -> Result<Live, Error<P>> {
        const POINT: &str = "UPDATE site_live SET previous_id = deploy_id, deploy_id = ?, paused = 1, set_at = ? \
            WHERE site_id = ? AND deploy_id != ? AND EXISTS (SELECT 1 FROM deploy WHERE site_id = ? AND id = ?)";
        const HISTORY: &str = "INSERT INTO publication (site_id, deploy_id, previous_id, cause, at) \
            SELECT site_id, deploy_id, previous_id, 'rollback', set_at FROM site_live \
            WHERE site_id = ? AND deploy_id = ? AND set_at = ? AND paused = 1";
        self.active(site).await?;
        let before = self.live(site).await?.ok_or(Refusal::NotLive { site })?;
        let (site_id, to_id, millis) = (site.to_string(), to.to_string(), at.as_millisecond());
        let batch = vec![
            Statement::prepare(POINT).bind(to_id.clone()).bind(millis).bind(site_id.clone()).bind(to_id.clone()).bind(site_id.clone()).bind(to_id.clone()),
            Statement::prepare(HISTORY).bind(site_id).bind(to_id).bind(millis),
            Event::DeployRolledBack { deploy: to, previous: before.deploy }.moved(site, at, to).map_err(Error::Log)?,
        ];
        let outcomes = self.ports.db().batch(batch).await?;
        if outcomes.first().is_some_and(|outcome| outcome.changes == 1) {
            return self.live(site).await?.ok_or_else(|| Refusal::NotLive { site }.into());
        }
        if before.deploy == to {
            return Err(Refusal::AlreadyLive { site, deploy: to }.into());
        }
        Err(Refusal::NoDeploy { site, deploy: to }.into())
    }

    /// Lets builds publish again after a rollback paused them. It does not publish anything
    /// itself: the next deploy goes live.
    pub async fn resume(&self, site: Id<SiteKind>, at: Timestamp) -> Result<Resumed, Error<P>> {
        const RESUME: &str = "UPDATE site_live SET paused = 0 WHERE site_id = ? AND paused = 1";
        const EVENT: &str = "INSERT INTO event (site_id, at, name, body) SELECT ?, ?, ?, ? \
            WHERE EXISTS (SELECT 1 FROM site_live WHERE site_id = ? AND paused = 1)";
        self.active(site).await?;
        let resumed = Event::SiteResumed;
        let batch = vec![
            Statement::prepare(EVENT)
                .bind(site.to_string())
                .bind(at.as_millisecond())
                .bind(resumed.name())
                .bind(serde_json::to_string(&resumed).map_err(Error::Log)?)
                .bind(site.to_string()),
            Statement::prepare(RESUME).bind(site.to_string()),
        ];
        let outcomes = self.ports.db().batch(batch).await?;
        Ok(if outcomes.get(1).is_some_and(|outcome| outcome.changes == 1) { Resumed::Resumed } else { Resumed::NotPaused })
    }
}
