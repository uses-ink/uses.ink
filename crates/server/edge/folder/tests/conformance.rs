//! The folder adapters against the edge's conformance suite. A folder has no suspensions and no custom
//! domains, so those two cases are reported as not offered.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]
#![allow(clippy::unused_async_trait_impl, reason = "the trait is async and a folder reads synchronously")]

extern crate uses_ink_edge_folder as edge_folder;

use std::cell::Cell;
use std::path::PathBuf;

use edge::{Edge, Limits};
use edge_conformance::{Case, Harness, Offers, Outcome, Seed, Unseeded};
use edge_folder::{Deploys, Files, Mode};
use model::Manifest;

/// A folder with one folder of deploy per host.
struct Folders {
    root: tempfile::TempDir,
    sites: Deploys,
    written: Cell<u32>,
}

fn block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

impl Folders {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let sites = Deploys::new(root.path(), Mode::Folders).unwrap();
        Self { root, sites, written: Cell::new(0) }
    }
}

impl Harness for Folders {
    type Scope = <Files as edge::Blobs>::Scope;
    type Sites = Deploys;
    type Blobs = Files;

    fn sites(&self) -> &Deploys {
        &self.sites
    }

    fn blobs(&self) -> &Files {
        &Files
    }

    fn edge(&self) -> Result<Edge<Deploys, Files>, edge::Error> {
        Edge::new(Deploys::new(self.root.path(), Mode::Folders).unwrap(), Files, Limits::default())
    }

    fn offers(&self) -> Offers {
        Offers::default()
    }

    async fn publish(&self, seed: &Seed) -> Result<Manifest, Unseeded> {
        let dir: PathBuf = self.root.path().join(&seed.host);
        std::fs::create_dir_all(&dir).map_err(Unseeded::because("make the folder"))?;
        let mut files = Vec::new();
        for page in &seed.pages {
            self.written.set(self.written.get() + 1);
            let name = format!("f{}.bin", self.written.get());
            std::fs::write(dir.join(&name), &page.bytes).map_err(Unseeded::because("write a file"))?;
            files.push(serde_json::json!({ "path": name, "hash": model::Hash::of(&page.bytes) }));
        }
        let manifest = seed.manifest();
        let written = serde_json::json!({
            "schema": "uses.manifest/v3",
            "deploy": manifest.id(),
            "files": files,
            "manifest": manifest,
            "record": {},
        });
        let text = serde_json::to_vec(&written).unwrap();
        std::fs::write(dir.join(".ink-manifest.json"), text).map_err(Unseeded::because("write the manifest"))?;
        Ok(manifest)
    }

    async fn suspend(&self, _host: &str) -> Result<(), Unseeded> {
        unreachable!("a folder offers no suspensions")
    }

    async fn alias(&self, _host: &str, _custom: &str) -> Result<(), Unseeded> {
        unreachable!("a folder offers no domains")
    }
}

edge_conformance::suite!(Folders::new, block);

#[test]
fn a_folder_offers_neither_suspensions_nor_domains() {
    let not_offered: Vec<&str> = Case::ALL
        .iter()
        .filter(|case| matches!(block(case.run(&Folders::new())), Outcome::NotOffered(_)))
        .map(|case| case.name())
        .collect();
    assert_eq!(
        not_offered,
        ["answers_403_and_no_bytes_for_a_suspended_site", "serves_a_custom_domain_with_its_sites_deploy"]
    );
}
