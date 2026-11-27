//! A tree an upload sent, read into a snapshot: the tree and every file it lists are blobs of the
//! site's tenant, put there by the API before the build was requested.

use blob::{Blob, Chunks, Tenant};
use build::Tree;
use host_native::{Memory, Read};
use model::{
    Ability, AnyCode, Entry, FileFacts, Gap, Hash, Known, Lack, Observed, Provenance, Snapshot, UnknownReason,
    UploadId, Version,
};

use crate::Error;

/// The blobs of one site's tenant, read as the files of an upload.
#[derive(Debug)]
pub(crate) struct Bundle<'a, B> {
    pub(crate) blobs: &'a B,
    pub(crate) tenant: &'a Tenant,
}

impl<B: Blob> Bundle<'_, B> {
    /// The bytes of `hash`, whole: an upload's file is as large as the operator's file limit lets
    /// it be, which is the most memory a read of one spends.
    async fn bytes(&self, hash: Hash) -> Result<Vec<u8>, Error<B::Error>> {
        let held = self.blobs.get(self.tenant, hash).await.map_err(Error::Stored)?;
        let held = held.ok_or(Error::Absent { blob: hash })?;
        held.all().await.map_err(Error::Stored)
    }

    /// Reads the tree `tree` names and each file of it into `store`, as a snapshot.
    pub(crate) async fn read(&self, tree: Hash, store: &mut Memory) -> Result<Read, Error<B::Error>> {
        let listed = Tree::read(&self.bytes(tree).await?).map_err(Error::Tree)?;
        let mut entries = Vec::new();
        let mut facts = Vec::new();
        for leaf in listed.files() {
            let held = store.insert(self.bytes(leaf.hash).await?);
            let bound = match leaf.modified {
                Some(value) => Known::NotAfter { value, from: Provenance::Filesystem },
                None => Known::Unknown { reason: UnknownReason::NotReported },
            };
            facts.push(FileFacts::new(leaf.path.clone(), bound.clone(), bound));
            entries.push(Entry::new(leaf.path.clone(), held, leaf.size));
        }
        let observed =
            Observed::new(vec![Ability::Snapshot], vec![Lack::new(Ability::History, AnyCode::Date, Gap::Absent)]);
        let id = UploadId::parse(tree.to_string()).map_err(|_| Error::Absent { blob: tree })?;
        let snapshot = Snapshot::new(Version::Upload(id), entries, facts, observed).map_err(Error::Snapshot)?;
        Ok(Read::new(snapshot))
    }
}
