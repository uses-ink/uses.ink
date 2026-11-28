//! The components `ink` carries (decision 0054): `build --check` runs them with no folder, says
//! so, and a folder replaces one of them only when it holds a file of that name.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_cli as cli;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use cli::{Cli, Exit};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/blog")
}

fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-bundle").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// `ink --json build --check <blog> <extra...>`: the exit and the one document.
fn check(cache: &Path, extra: &[&OsStr]) -> (Exit, serde_json::Value) {
    let blog = fixture();
    let mut args: Vec<&OsStr> = vec![
        "ink".as_ref(),
        "--json".as_ref(),
        "build".as_ref(),
        "--check".as_ref(),
        "-C".as_ref(),
        blog.as_os_str(),
        "--cache".as_ref(),
        cache.as_os_str(),
    ];
    args.extend_from_slice(extra);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = Cli::start(args, &mut out, &mut err).unwrap();
    (exit, serde_json::from_slice(&out).unwrap())
}

/// Whether this `ink` was built with its components (`cargo xtask component` ran first).
fn bundled() -> bool {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/plugin/bundle.json").exists()
}

#[test]
fn the_bundled_components_build_the_same_deploy() {
    let cache = scratch("bundled");
    let (exit, document) = check(&cache, &[]);
    if !bundled() {
        assert_eq!(document["error"], "component.bundled.missing", "{document}");
        return;
    }
    assert_eq!(exit, Exit::Done, "{document}");
    assert_eq!(document["native"], document["component"]);
    let used = document["components"].as_array().unwrap();
    assert_eq!(used.len(), 2, "{document}");
    for component in used {
        assert_eq!(component["origin"], "bundled");
        assert_eq!(component["hash"].as_str().unwrap().len(), 64);
        assert!(component["version"].is_string());
    }
}

#[test]
fn a_folder_replaces_only_what_it_holds() {
    if !bundled() {
        return;
    }
    let folder = scratch("folder");
    let theme = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/plugin/theme-quiet.wasm");
    std::fs::copy(theme, folder.join("theme-quiet.wasm")).unwrap();
    let (exit, document) = check(&scratch("folder-cache"), &["--components".as_ref(), folder.as_os_str()]);
    assert_eq!(exit, Exit::Done, "{document}");
    let used = document["components"].as_array().unwrap();
    let origin = |id: &str| used.iter().find(|c| c["id"] == id).unwrap()["origin"].as_str().unwrap().to_owned();
    assert_eq!(origin("ink:format-markdown"), "bundled");
    assert!(origin("ink:theme-quiet").ends_with("theme-quiet.wasm"));
    let version = used.iter().find(|c| c["id"] == "ink:theme-quiet").unwrap();
    assert!(version["version"].is_null(), "a folder's file has no known version");
}
