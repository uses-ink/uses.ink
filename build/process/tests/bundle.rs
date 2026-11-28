//! The local builder over an uploaded tree: the tree and its files are blobs of the tenant.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_build_local as build_local;

use blob::{Blob, Tenant};
use blob_dir::Directory;
use build::{Build, BuildId, Event, Leaf, Outcome, Request, Source, Tree, Trigger};
use build_local::Local;
use model::{Bytes, ContentPath, ExternalUrl, Hash};

const TENANT: &str = "0123456789abcdef0123456789abcdef";

/// Puts `text` in the tenant's blobs and names it as a leaf at `path`.
async fn leaf(blobs: &Directory, tenant: &Tenant, path: &str, text: &str) -> Leaf {
    let hash = Hash::of(text);
    blobs.put(tenant, hash, [text.as_bytes()]).await.unwrap();
    Leaf { path: ContentPath::parse(path).unwrap(), hash, size: Bytes(text.len() as u64), modified: None }
}

fn request(tree: Hash) -> Request {
    Request {
        build: BuildId::from_bytes([3; 16]),
        tenant: TENANT.parse().unwrap(),
        trigger: Trigger::Upload,
        source: Source::Bundle(tree),
        base: ExternalUrl::parse("https://cstef.uses.ink").unwrap(),
    }
}

#[test]
fn an_uploaded_tree_builds_like_its_folder() {
    pollster::block_on(async {
        let folder = tempfile::tempdir().unwrap();
        let blobs = Directory::open(folder.path()).unwrap();
        let tenant: Tenant = TENANT.parse().unwrap();
        let page = leaf(&blobs, &tenant, "index.md", "# Hello\n\nOne page.\n").await;
        let tree = Tree::of(vec![page]).unwrap();
        let hash = tree.hash();
        blobs.put(&tenant, hash, [tree.bytes()]).await.unwrap();
        let builder = Local::new(Directory::open(folder.path()).unwrap());
        let mut events: Vec<Event> = Vec::new();
        let outcome = builder.run(&request(hash), &mut events).await.unwrap();
        let Outcome::Built(built) = outcome else { panic!("not built: {events:?}") };
        assert_ne!(built.manifest.entries().len(), 0);
        assert!(events.iter().any(|event| matches!(event, Event::SourceFetched { files: 1, .. })), "{events:?}");
    });
}

#[test]
fn a_tree_whose_file_is_not_held_is_a_failure_of_the_builder() {
    pollster::block_on(async {
        let folder = tempfile::tempdir().unwrap();
        let blobs = Directory::open(folder.path()).unwrap();
        let tenant: Tenant = TENANT.parse().unwrap();
        let missing =
            Leaf { path: ContentPath::parse("a.md").unwrap(), hash: Hash::of("x"), size: Bytes(1), modified: None };
        let tree = Tree::of(vec![missing]).unwrap();
        blobs.put(&tenant, tree.hash(), [tree.bytes()]).await.unwrap();
        let builder = Local::new(Directory::open(folder.path()).unwrap());
        let mut events: Vec<Event> = Vec::new();
        let failed = builder.run(&request(tree.hash()), &mut events).await;
        assert!(failed.is_err(), "{failed:?}");
    });
}
