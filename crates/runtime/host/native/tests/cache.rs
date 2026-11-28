//! The cache on disk through the public API, in throwaway folders under cargo's target
//! directory: what is put comes back, nothing is written until something is put, a corrupt
//! entry or value is dropped and said, the bound is kept, and no read takes the cache as content.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_native as host_native;

use std::path::{Path, PathBuf};

use engine::Cache;
use engine::cache::{Key, Kind};
use host_native::{Disk, Folder, Memory, Skip};
use model::{Bytes, Hash, Notice};

fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-native-cache").join(name);
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    root
}

fn key(name: &str) -> Key {
    Key::new(Kind::Image, Hash::of(name))
}

fn codes(notices: &[Notice]) -> Vec<&'static str> {
    notices.iter().map(|notice| notice.announcement.code()).collect()
}

#[test]
fn what_is_put_comes_back_in_order_and_from_another_opening() {
    let root = scratch("round");
    let disk = Disk::open(&root);
    assert!(!root.exists(), "nothing is written before something is put");
    assert_eq!(disk.get(&key("a")), None);
    disk.put(&key("a"), &[b"record", b"copy", b"copy"]);
    assert_eq!(disk.get(&key("a")), Some(vec![b"record".to_vec(), b"copy".to_vec(), b"copy".to_vec()]));
    assert_eq!(codes(&disk.notices()), ["render.cache.cold"]);
    let again = Disk::open(&root);
    assert_eq!(again.get(&key("a")).unwrap().len(), 3);
    again.put(&key("b"), &[b"other"]);
    assert!(again.notices().is_empty(), "a cache already made is not cold");
    // the two copies are one value
    assert_eq!(std::fs::read_dir(root.join("value")).unwrap().count(), 3);
}

#[test]
fn a_corrupt_value_or_entry_is_dropped_and_said() {
    let root = scratch("corrupt");
    let disk = Disk::open(&root);
    disk.put(&key("a"), &[b"record"]);
    disk.put(&key("b"), &[b"other"]);
    disk.notices();
    std::fs::write(root.join("value").join(Hash::of(b"record").to_string()), b"recorD").unwrap();
    assert_eq!(disk.get(&key("a")), None);
    let entry = root.join("key/image").join(Hash::of("b").to_string());
    let mut text = std::fs::read(&entry).unwrap();
    text[25] ^= 1;
    std::fs::write(&entry, text).unwrap();
    assert_eq!(disk.get(&key("b")), None);
    let notices = disk.notices();
    assert_eq!(codes(&notices), ["cache.entry.corrupt", "cache.entry.corrupt"]);
    assert!(!entry.exists(), "the corrupt entry is removed");
    disk.put(&key("a"), &[b"record"]);
    assert_eq!(disk.get(&key("a")), Some(vec![b"record".to_vec()]), "the bad value was removed with its entry");
    disk.reject(&key("a"));
    assert_eq!(disk.get(&key("a")), None);
    assert_eq!(codes(&disk.notices()), ["cache.entry.corrupt"]);
}

#[test]
fn the_sweep_drops_the_entries_used_longest_ago() {
    let root = scratch("sweep");
    let mut disk = Disk::open(&root);
    disk.bound = Bytes(25);
    for name in ["old", "middle", "new"] {
        disk.put(&key(name), &[name.repeat(4).as_bytes()]);
        let entry = root.join("key/image").join(Hash::of(name).to_string());
        let at = match name {
            "old" => 1_000,
            "middle" => 2_000,
            _ => 3_000,
        };
        let when = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(at);
        std::fs::File::options().write(true).open(entry).unwrap().set_modified(when).unwrap();
    }
    disk.notices();
    // a hit is a use: `old` is now the newest
    assert!(disk.get(&key("old")).is_some());
    disk.sweep();
    assert!(disk.get(&key("middle")).is_none());
    assert!(disk.get(&key("old")).is_some());
    assert!(disk.get(&key("new")).is_some());
    assert_eq!(std::fs::read_dir(root.join("value")).unwrap().count(), 2);
    assert_eq!(codes(&disk.notices()), ["cache.entry.evicted"]);
}

/// A sweep that cannot remove a file says so, with the system's error, and counts it.
#[test]
#[cfg(unix)]
fn a_file_the_sweep_cannot_remove_is_said_and_counted() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = scratch("stays");
    let mut disk = Disk::open(&root);
    disk.bound = Bytes(1);
    for name in ["one", "two"] {
        disk.put(&key(name), &[name.repeat(4).as_bytes()]);
    }
    let folder = root.join("key/image");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).unwrap();
    // an account that may write anywhere (root) cannot make a file stay
    let writable = std::fs::File::create(folder.join("probe")).is_ok();
    let capture = logging::Capture::new();
    capture.run(|| disk.sweep());
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    if writable {
        return;
    }
    assert_eq!(disk.stayed(), 2, "both entries stay");
    let said = capture.named(logging::event::CACHE_STAYED);
    assert_eq!(said.len(), 1, "one event per sweep");
    assert_eq!(said[0].number("count"), Some(2));
    assert_eq!(said[0].level, logging::Level::WARN);
    assert!(said[0].field("error").and_then(logging::Value::chain).is_some(), "the error keeps its chain");
}

#[test]
fn no_read_takes_the_cache_as_content() {
    let root = scratch("site");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("index.md"), "# Hello\n").unwrap();
    let disk = Disk::open(root.join(".ink/cache"));
    disk.put(&key("a"), &[b"value"]);
    let read = Folder::new(&root).read(&mut Memory::default()).unwrap();
    let paths: Vec<&str> = read.snapshot.tree().iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, ["index.md"]);
    assert_eq!(read.skipped.len(), 1);
    assert_eq!(read.skipped[0].path.as_str(), ".ink/cache");
    assert_eq!(read.skipped[0].reason, Skip::Cache);
}
