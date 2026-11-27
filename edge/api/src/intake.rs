//! Intake: an archive of a built folder, checked and made a deploy.
//!
//! What `ink build` writes (the files and `.ink-manifest.json`) is sent as a tar archive. The
//! archive is read within the operator's limits, the manifest is checked as the folder adapter
//! checks one (its schema, its deploy id, its routes), every file the manifest lists must be in
//! the archive and hash to what the manifest says, and only then are the blobs stored under the
//! site's tenant and the control plane records the deploy. Nothing is stored for an archive that
//! fails a check.

use std::collections::BTreeMap;

use blob::{Blob, Tenant};
use clock::{Clock, Draw};
use control::{Ports, Site};
use edge_folder::{MANIFEST, Written};
use model::{ContentPath, Hash};

use crate::Api;
use crate::credential::Credentials;
use crate::fault::{Fault, Intake};
use crate::tar::{self, Tree};
use crate::view::Deployed;

/// An archive taken apart: the files it holds and the deploy its manifest describes.
struct Parts {
    tree: Tree,
    checked: edge_folder::Checked,
    /// The file that holds each blob.
    placed: BTreeMap<Hash, ContentPath>,
}

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw> Api<P, C, K, D> {
    /// Reads `body` as a built folder and makes it a deploy of `site`, live unless publishing is
    /// paused.
    pub(crate) async fn deploy(&self, site: &Site, body: &[u8]) -> Result<Deployed, Fault<P>> {
        let parts = Self::open(body, &self.limits.archives())?;
        let tenant = Tenant::try_from(site.id).map_err(control::Refusal::Tenant)?;
        for entry in parts.checked.manifest.entries() {
            self.store(&tenant, &parts, entry.blob()).await?;
        }
        let files = parts.tree.len() as u64;
        let new =
            control::NewDeploy { site: site.id, manifest: parts.checked.manifest, build: None, at: self.clock.now() };
        let recorded = self.control.deploy(new).await?;
        Ok(Deployed::of(&recorded, files))
    }

    /// The archive read, its manifest checked, and every listed file found and hashed.
    fn open(body: &[u8], limits: &tar::Limits) -> Result<Parts, Fault<P>> {
        let tree = Tree::read(body, limits).map_err(Fault::Archive)?;
        let manifest = ContentPath::parse(MANIFEST).map_err(|_| Fault::Intake(Intake::NoManifest))?;
        let bytes = tree.get(&manifest).ok_or(Fault::Intake(Intake::NoManifest))?;
        let written: Written = serde_json::from_slice(bytes)
            .map_err(|source| edge_folder::Error::Manifest { path: MANIFEST.into(), source })
            .map_err(Fault::Folder)?;
        let checked = written.check(std::path::Path::new(MANIFEST)).map_err(Fault::Folder)?;
        for (listed, path) in &checked.files {
            let bytes = tree.get(path).ok_or_else(|| Intake::Absent { path: path.as_str().to_owned() })?;
            let found = Hash::of(bytes);
            if found != *listed {
                let (path, listed) = (path.as_str().to_owned(), *listed);
                return Err(Fault::Intake(Intake::Mismatch { path, listed, found }));
            }
        }
        let placed: BTreeMap<Hash, ContentPath> =
            checked.files.iter().map(|(hash, path)| (*hash, path.clone())).collect();
        if let Some(entry) = checked.manifest.entries().iter().find(|entry| !placed.contains_key(&entry.blob())) {
            return Err(Fault::Folder(edge_folder::Error::Unplaced { route: entry.route().clone() }));
        }
        Ok(Parts { tree, checked, placed })
    }

    /// Stores a blob of the manifest, read from the file that holds it.
    async fn store(&self, tenant: &Tenant, parts: &Parts, hash: Hash) -> Result<(), Fault<P>> {
        let bytes = parts.placed.get(&hash).and_then(|path| parts.tree.get(path)).unwrap_or_default();
        let stored = self.control.ports().blob().put(tenant, hash, [bytes]).await;
        stored.map_err(|error| Fault::Control(error.into()))?;
        Ok(())
    }
}
