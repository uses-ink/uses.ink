//! The digest of a job's inputs.

use super::*;

struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("farm-inputs-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn put(&self, path: &str, text: &str) {
        let full = self.0.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }

    fn digest(&self, files: Option<&[&str]>) -> Hash {
        let inputs = Inputs::new(Arc::new(Walk), &[]).unwrap();
        let files = files
            .map(|globs| Selection::parse(&globs.iter().map(|glob| (*glob).to_owned()).collect::<Vec<_>>()).unwrap());
        inputs.digest(&self.0, files.as_ref(), true).unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn the_digest_follows_the_selected_files_and_the_forced_ones() {
    let tree = Tree::new("follow");
    tree.put("src/lib.rs", "fn a() {}");
    tree.put("docs/a.md", "one");
    tree.put("Cargo.lock", "lock");
    let docs_free = Some(&["**", "!docs/**"][..]);
    let (all, narrow) = (tree.digest(None), tree.digest(docs_free));
    assert_eq!(all, tree.digest(None), "the same tree, the same digest");
    tree.put("docs/a.md", "two");
    assert_ne!(all, tree.digest(None), "a byte of an input changed it");
    assert_eq!(narrow, tree.digest(docs_free), "a file outside the inputs did not");
    tree.put("src/lib.rs", "fn b() {}");
    assert_ne!(narrow, tree.digest(docs_free));
    let narrow = tree.digest(docs_free);
    tree.put("Cargo.lock", "newer lock");
    assert_ne!(narrow, tree.digest(docs_free), "Cargo.lock is read by every job");
    let narrow = tree.digest(docs_free);
    tree.put("target/debug/x", "build output");
    tree.put(".env", "SECRET=1");
    assert_eq!(narrow, tree.digest(docs_free), "what the sync leaves out is not read");
}

#[test]
fn a_second_digest_reads_no_file_that_did_not_change() {
    let tree = Tree::new("stat");
    tree.put("a.rs", "one");
    let old = UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    std::fs::File::options().write(true).open(tree.0.join("a.rs")).unwrap().set_modified(old).unwrap();
    let first = tree.digest(None);
    let cache = std::fs::read_to_string(Hashes::file(&tree.0)).unwrap();
    assert!(cache.starts_with("a.rs\t3\t"), "{cache}");
    // the stored hash is believed while the stat holds: poison it and the digest follows
    let (head, _) = cache.trim_end().rsplit_once('\t').unwrap();
    let poisoned = format!("{head}\t{:032x}\n", 7);
    std::fs::write(Hashes::file(&tree.0), poisoned).unwrap();
    assert_ne!(first, tree.digest(None), "a matching stat is trusted");
    tree.put("a.rs", "two");
    assert_eq!(tree.digest(None), tree.digest(None));
}

#[test]
fn the_toolchain_channel_is_read_from_its_file() {
    let tree = Tree::new("channel");
    assert_eq!(Toolchain::channel(&tree.0).unwrap(), None);
    tree.put("rust-toolchain.toml", "[toolchain]\nchannel = \"1.100.0\"\n");
    assert_eq!(Toolchain::channel(&tree.0).unwrap().as_deref(), Some("1.100.0"));
    tree.put("rust-toolchain.toml", "[toolchain\n");
    assert!(Toolchain::channel(&tree.0).is_err());
}
