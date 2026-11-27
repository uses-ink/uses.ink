//! `ink init`: link this folder to a site, without publishing it.

use std::path::PathBuf;

use clap::Args;
use hyper::Method;

use super::link::Link;
use super::view::Linked;
use crate::deploy::wire::{Overview, Site};
use crate::deploy::{Failure, Reach, Remote};
use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;
use crate::settings::{Preferences, Process};
use crate::stage::{StageLine, Verb};

/// `ink init [--handle NAME | --site ID]`.
#[derive(Debug, Args)]
pub struct Init {
    /// Make a site with this handle, built from what `ink push` sends, and link the folder to it.
    #[arg(long, value_name = "NAME", conflicts_with = "site")]
    pub handle: Option<String>,
    /// Replace a link the folder has to another site.
    #[arg(long)]
    pub force: bool,
    /// The site to link when the link file does not say; the API's address and the site's id come
    /// from the flags and the variables, never from the config file's `[account]` table.
    #[command(flatten)]
    pub remote: Remote,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
}

impl Init {
    /// The site to link: the one `wanted` names, or a new one made with `--handle`.
    fn site(&self, reach: &Reach, wanted: Option<&str>) -> Result<(Site, bool), Failure> {
        let unreadable = |source| Failure::Unreadable { source };
        match (wanted, &self.handle) {
            (Some(id), _) => {
                let id = id
                    .parse::<control::Id<control::SiteKind>>()
                    .map_err(|source| Failure::Site { written: id.to_owned(), source })?;
                let body = reach.block(reach.wire.repeat(Method::GET, &format!("/v1/sites/{id}"), Vec::new()))?;
                let overview: Overview = serde_json::from_slice(&body).map_err(unreadable)?;
                Ok((overview.site, false))
            }
            (None, Some(handle)) => {
                let body = serde_json::json!({ "handle": handle, "source": { "kind": "upload" } });
                let body = serde_json::to_vec(&body).map_err(unreadable)?;
                let made = reach.block(reach.wire.call(Method::POST, "/v1/sites", body))?;
                Ok((serde_json::from_slice(&made).map_err(unreadable)?, true))
            }
            (None, None) => Err(Failure::Nameless),
        }
    }

    /// Links the folder: to the site `--site` names, or to a site made with `--handle`.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        let held = Link::find(&self.dir)?;
        let reach = self.remote.reach(held.as_ref(), &Process, &preferences)?;
        let wanted = match (&self.handle, &self.remote.site) {
            (None, Some(site)) => Some(site.clone()),
            (Some(_), None) => None,
            _ => return Err(Failure::Nameless.into()),
        };
        if let Some(link) = &held {
            let same = wanted.as_deref() == Some(link.site.as_str());
            if !same && !self.force {
                return Err(Failure::Linked { path: Link::path(&self.dir), site: link.site.clone() }.into());
            }
        }
        let (site, created) = self.site(&reach, wanted.as_deref())?;
        let link = Link::new(reach.address.clone(), site.id.clone(), Some(site.handle.clone()));
        let unchanged = held.as_ref() == Some(&link);
        let file = if unchanged { Link::path(&self.dir) } else { link.save(&self.dir)? };
        io.stage(&StageLine::new(Verb::Linked, format!("{} to site {}", self.dir.display(), site.handle)))?;
        let linked = Linked {
            schema: Linked::SCHEMA,
            site: site.id,
            handle: site.handle,
            url: site.url,
            api: reach.address.clone(),
            file,
            created,
            unchanged,
        };
        io.result(&linked)?;
        io.json(&linked)?;
        Ok(Exit::Done)
    }
}
