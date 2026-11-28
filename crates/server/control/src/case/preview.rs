//! Previews of branches (decision 0091): labelled deploys that serve at their own address, are
//! listed to the owner only, and end by themselves.

use std::collections::BTreeSet;

use blob::Blob;
use build::Source;
use db::{Db, Statement};
use jiff::Timestamp;
use model::{Bytes, DeployId, Hash};
use sea_query::{Expr, ExprTrait, Order, Query};

use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::{
    Control, Error, Event, Handle, NewPreview, Ports, Preview, Purpose, Refusal, SiteState, Slug, Spent, Swept, table,
};

/// The blobs of a deploy that went, which an abandon queues unless something else names them.
#[derive(Debug)]
struct Leaving<'a> {
    candidates: BTreeSet<Hash>,
    ignoring: &'a [Slug],
    at: Timestamp,
}

/// What a preview host serves: the site and the deploy of the preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Showing {
    /// The site.
    pub site: Id<SiteKind>,
    /// The deploy the preview serves.
    pub deploy: DeployId,
}

impl<P: Ports> Control<P> {
    /// Records a preview of a branch: `new.manifest` served at `<slug>--<handle>` until the
    /// policy's time to live has passed, in place of an earlier preview with the same label.
    ///
    /// Only a site with a public git source has one: an upload or a folder has no branches, and a
    /// site with a git token is private, so its branches are never served at an address anyone
    /// can open. The deploy is not recorded as one: the live pointer, the deploy list and
    /// rollback never see it. Its blobs count against the account's storage, and one that would
    /// take the account over the limit is refused whole.
    pub async fn preview(&self, new: NewPreview) -> Result<Preview, Error<P>> {
        let site = self.active(new.site).await?;
        self.previewable(&site).await?;
        let tenant = Self::tenant(site.id)?;
        let bytes = self.stat(&tenant, &new.manifest).await?;
        let limit = self.policy.limits.stored;
        let total = self.stored(site.account).await?;
        if total > limit {
            self.discard(site.id, &new.manifest, new.at).await?;
            return Err(self.policy.limits.over(Spent::Stored, total.count()).into());
        }
        let id = new.manifest.id();
        self.ports.blob().put(&tenant, id.hash(), [new.manifest.canonical()]).await?;
        let before = self.preview_of(site.id, &new.slug).await?;
        let life = i64::try_from(self.policy.preview.as_millis()).unwrap_or(i64::MAX);
        let ends = new.at.as_millisecond().saturating_add(life);
        let preview = Preview {
            site: site.id,
            slug: new.slug.clone(),
            deploy: id,
            files: new.manifest.entries().len() as u64,
            bytes,
            created: new.at,
            expires: Timestamp::from_millisecond(ends).unwrap_or(Timestamp::MAX),
        };
        let event = Event::PreviewPushed { slug: new.slug, deploy: id }.insert(site.id, new.at)?;
        let mut batch = vec![preview.replace()?, event];
        batch.extend(Self::revive(site.id, &new.manifest));
        self.ports.db().batch(batch).await?;
        if let Some(old) = before.filter(|old| old.deploy != id) {
            let candidates = self.reach(site.id, old.deploy).await?;
            self.abandon(site.id, Leaving { candidates, ignoring: &[], at: new.at }).await?;
            self.reap_site(site.id, new.at, &BTreeSet::new()).await?;
        }
        Ok(preview)
    }

    /// Whether the site has branches to preview: a git source, with no credential, so that what a
    /// preview serves at an address anyone can open was public to begin with.
    pub(crate) async fn previewable(&self, site: &crate::Site) -> Result<(), Error<P>> {
        if !matches!(site.source, Source::Git { .. }) {
            return Err(Refusal::NoPreview { site: site.id }.into());
        }
        if self.credentials(site.id).await?.iter().any(|credential| credential.purpose == Purpose::GitToken) {
            return Err(Refusal::Private { site: site.id }.into());
        }
        Ok(())
    }

    /// Ends the preview of `site` labelled `slug` now, and queues the blobs only it named, which
    /// are deleted once their grace is over (decision 0150). A blob a deploy or another preview
    /// still names stays.
    /// Removing what is not there is `preview.label.unknown`, so a typo is not a success.
    pub async fn remove_preview(&self, site: Id<SiteKind>, slug: &Slug, at: Timestamp) -> Result<Swept, Error<P>> {
        self.site(site).await?;
        let Some(found) = self.preview_of(site, slug).await? else {
            return Err(Refusal::NoLabel { site, label: slug.to_string() }.into());
        };
        let candidates = self.reach(site, found.deploy).await?;
        self.abandon(site, Leaving { candidates, ignoring: std::slice::from_ref(slug), at }).await?;
        let delete = Query::delete()
            .from_table(table::Preview::Table)
            .and_where(Expr::col(table::Preview::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Preview::Slug).eq(slug.as_str()))
            .to_owned();
        let event = Event::PreviewExpired { slug: slug.clone() }.insert(site, at)?;
        self.ports.db().batch(vec![delete.into(), event]).await?;
        let freed = self.reap_site(site, at, &BTreeSet::new()).await?;
        Ok(Swept { previews: 1, ..freed })
    }

    /// The previews of `site` that have not ended at `at`, newest first.
    pub async fn previews(&self, site: Id<SiteKind>, at: Timestamp) -> Result<Vec<Preview>, Error<P>> {
        self.site(site).await?;
        let select = Query::select()
            .columns(Preview::COLUMNS)
            .from(table::Preview::Table)
            .and_where(Expr::col(table::Preview::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Preview::ExpiresAt).gt(at.as_millisecond()))
            .order_by(table::Preview::CreatedAt, Order::Desc)
            .order_by(table::Preview::Slug, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(Preview::read).collect::<Result<_, _>>()?)
    }

    /// What the host `<slug>--<handle>` serves at `at`, or `None` when the handle names no site
    /// that takes changes and is not suspended, or the site has no such preview, or it ended.
    pub async fn previewed(&self, preview: (&Slug, &Handle), at: Timestamp) -> Result<Option<Showing>, Error<P>> {
        let (slug, handle) = preview;
        let Some(site) = self.by_handle(handle).await?.filter(|site| site.state == SiteState::Active) else {
            return Ok(None);
        };
        if self.suspension(site.id).await?.is_some() {
            return Ok(None);
        }
        Ok(self
            .preview_of(site.id, slug)
            .await?
            .filter(|found| !found.is_over(at))
            .map(|found| Showing { site: site.id, deploy: found.deploy }))
    }

    /// Removes every preview that has ended at `at`, queues the blobs only they named and deletes
    /// the queued blobs of every site whose grace is over (decision 0150). A blob a deploy or
    /// another preview of the site still names stays.
    pub async fn expire(&self, at: Timestamp) -> Result<Swept, Error<P>> {
        let select = Query::select()
            .columns(Preview::COLUMNS)
            .from(table::Preview::Table)
            .and_where(Expr::col(table::Preview::ExpiresAt).lte(at.as_millisecond()))
            .order_by(table::Preview::SiteId, Order::Asc)
            .order_by(table::Preview::Slug, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        let ended: Vec<Preview> = rows.iter().map(Preview::read).collect::<Result<_, _>>()?;
        let mut swept = Swept::default();
        let sites: BTreeSet<Id<SiteKind>> = ended.iter().map(|preview| preview.site).collect();
        for site in sites {
            let these: Vec<&Preview> = ended.iter().filter(|preview| preview.site == site).collect();
            let gone: Vec<Slug> = these.iter().map(|preview| preview.slug.clone()).collect();
            let mut candidates = BTreeSet::new();
            for preview in &these {
                candidates.extend(self.reach(site, preview.deploy).await?);
            }
            self.abandon(site, Leaving { candidates, ignoring: &gone, at }).await?;
            let mut batch: Vec<Statement> = Vec::new();
            for slug in &gone {
                batch.push(
                    Query::delete()
                        .from_table(table::Preview::Table)
                        .and_where(Expr::col(table::Preview::SiteId).eq(site.to_string()))
                        .and_where(Expr::col(table::Preview::Slug).eq(slug.as_str()))
                        .to_owned()
                        .into(),
                );
                batch.push(Event::PreviewExpired { slug: slug.clone() }.insert(site, at)?);
            }
            self.ports.db().batch(batch).await?;
            swept.previews += gone.len() as u64;
        }
        let freed = self.reap(at).await?;
        swept.blobs += freed.blobs;
        swept.bytes = Bytes(swept.bytes.count().saturating_add(freed.bytes.count()));
        Ok(swept)
    }

    /// Queues the `leaving` blobs that no deploy of `site` and no preview of it, but those
    /// labelled `ignoring`, reaches. The caller removes the rows that named them, then reaps.
    async fn abandon(&self, site: Id<SiteKind>, leaving: Leaving<'_>) -> Result<(), Error<P>> {
        let Leaving { candidates, ignoring, at } = leaving;
        let mut held = BTreeSet::new();
        for deploy in self.deploy_ids(site).await?.into_iter().chain(self.preview_ids(site, ignoring).await?) {
            held.extend(self.reach(site, deploy).await?);
        }
        let goners: BTreeSet<Hash> = candidates.difference(&held).copied().collect();
        self.condemn(site, &goners, at).await
    }

    /// The preview of `site` with this label, ended or not.
    async fn preview_of(&self, site: Id<SiteKind>, slug: &Slug) -> Result<Option<Preview>, Error<P>> {
        let select = Query::select()
            .columns(Preview::COLUMNS)
            .from(table::Preview::Table)
            .and_where(Expr::col(table::Preview::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Preview::Slug).eq(slug.as_str()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Preview::read(&row)).transpose()?)
    }

    /// The deploys the previews of `site` serve, but those labelled `ignoring`: what a sweep
    /// must keep.
    pub(crate) async fn preview_ids(&self, site: Id<SiteKind>, ignoring: &[Slug]) -> Result<Vec<DeployId>, Error<P>> {
        let select = Query::select()
            .columns([table::Preview::Slug, table::Preview::DeployId])
            .from(table::Preview::Table)
            .and_where(Expr::col(table::Preview::SiteId).eq(site.to_string()))
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        let mut ids = Vec::new();
        for row in &rows {
            let fields = Fields(row);
            let slug = fields.text(table::Preview::Slug)?;
            if !ignoring.iter().any(|ignored| ignored.as_str() == slug) {
                ids.push(fields.deploy(table::Preview::DeployId)?);
            }
        }
        Ok(ids)
    }
}
