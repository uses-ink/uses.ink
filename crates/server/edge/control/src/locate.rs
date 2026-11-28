//! Where a blob store keeps its files.

use std::path::PathBuf;

use blob::Tenant;
use blob_dir::Directory;

/// A blob store whose blobs are files the edge can read: the folder of a tenant holds one file
/// a blob, named by its hash. Only a store of this kind can be served through the folder
/// adapter's reads; one that is not (R2) is served by an adapter of its own.
pub trait Locate {
    /// The folder `tenant`'s blobs are in.
    fn folder(&self, tenant: &Tenant) -> PathBuf;
}

impl Locate for Directory {
    fn folder(&self, tenant: &Tenant) -> PathBuf {
        Self::folder(self, tenant)
    }
}
