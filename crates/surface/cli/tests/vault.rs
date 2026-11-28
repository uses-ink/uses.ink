//! An Obsidian vault becomes a site through the real binary: wikilinks, embeds, callouts and
//! images with variants and no EXIF, built twice to the same bytes, checked against the
//! components, and with a private note left out.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

use std::path::{Path, PathBuf};
use std::process::Command;

/// What one run said.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

fn ink(args: &[&str]) -> Ran {
    let out = Command::new(env!("CARGO_BIN_EXE_ink")).args(args).env_clear().output().unwrap();
    Ran {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-vault").join(name);
    std::fs::remove_dir_all(&path).ok();
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() { copy(&path, &target) } else { std::fs::copy(&path, &target).map(|_| ()).unwrap() }
    }
}

/// Every file under `root` with its bytes, in path order.
fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push((path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap()));
            }
        }
    }
    found.sort();
    found
}

/// The fixture vault with a photo that carries EXIF and a note that shows it.
fn vault(name: &str) -> PathBuf {
    let dir = scratch(name).join("vault");
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/vault"), &dir);
    let photo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/host/image/fixture/orientation.jpg");
    let bytes = std::fs::read(photo).unwrap();
    assert!(bytes.windows(4).any(|window| window == b"Exif"), "the fixture photo carries EXIF");
    std::fs::write(dir.join("attachments/photo.jpg"), bytes).unwrap();
    std::fs::write(
        dir.join("Photos.md"),
        "---\npublish: true\n---\n# Photos\n\nThe one photo: ![[photo.jpg]]\n\n> [!note] Taken at home\n> Back to [[Rust]].\n",
    )
    .unwrap();
    dir
}

#[test]
fn a_vault_builds_with_its_links_embeds_callouts_and_photos() {
    let dir = vault("build");
    let out = dir.parent().unwrap().join("out");
    let built =
        ink(&["build", "-C", dir.to_str().unwrap(), "-o", out.to_str().unwrap(), "--base", "https://ink.example"]);
    assert_eq!(built.code, 0, "{}{}", built.out, built.err);
    assert_eq!(built.out.trim().len(), 64, "stdout is the deploy id alone: {}", built.out);

    let page = |route: &str| std::fs::read_to_string(out.join(route).join("index.html")).unwrap();
    let home = std::fs::read_to_string(out.join("index.html")).unwrap();
    // wikilinks by name, in any case, by alias and by partial path
    for href in ["/notes/rust", "/notes/ownership"] {
        assert!(home.contains(&format!("href=\"{href}")), "{href} in the home page");
    }
    assert!(home.contains("the language"), "an alias gives the link its text");
    // embeds of a heading and of a block are filled in, and a callout keeps its fold
    assert!(home.contains("One mutable borrow or many shared ones."), "the embedded block");
    assert!(home.contains("Folded at first") && home.contains("<details"), "a folded callout");
    // the photo: both formats, orientation applied, no EXIF in any byte of the output
    let photos = page("photos");
    assert!(photos.contains("width=\"32\" height=\"64\""), "the stored orientation is applied: {photos}");
    assert!(photos.contains("image/avif"), "an AVIF source beside the fallback");
    let every = files(&out);
    let copies: Vec<_> = every
        .iter()
        .map(|(path, _)| path.to_string_lossy().into_owned())
        .filter(|path| path.starts_with("attachments/photo."))
        .collect();
    for extension in ["avif", "jpeg"] {
        assert!(copies.iter().any(|path| path.ends_with(extension)), "a {extension} copy among {copies:?}");
    }
    for (path, bytes) in &every {
        let kept = bytes.windows(4).any(|window| window == b"Exif") && path.starts_with("attachments");
        assert!(!kept, "{} keeps EXIF", path.display());
        if path.extension().is_some_and(|extension| extension == "html") {
            assert!(!String::from_utf8_lossy(bytes).contains("<script"), "{} runs script", path.display());
        }
    }
    // a note that was never selected is not published, and the build says what became public
    assert!(!out.join("private/index.html").exists(), "a private note is a page");
    assert!(built.err.contains("embed.target.cycle"), "the cycle is a warning with its code: {}", built.err);
}

#[test]
fn a_vault_builds_the_same_bytes_twice_and_agrees_with_its_components() {
    let dir = vault("twice");
    let root = dir.parent().unwrap().to_path_buf();
    let (one, two) = (root.join("one"), root.join("two"));
    let build = |out: &Path| {
        ink(&["build", "-C", dir.to_str().unwrap(), "-o", out.to_str().unwrap(), "--base", "https://ink.example"])
    };
    let (first, second) = (build(&one), build(&two));
    assert_eq!((first.code, second.code), (0, 0), "{}{}", first.err, second.err);
    assert_eq!(first.out, second.out, "the same deploy id");
    assert_eq!(files(&one), files(&two), "the same bytes");

    let checked = ink(&["build", "--check", "-C", dir.to_str().unwrap(), "-o", root.join("three").to_str().unwrap()]);
    if !Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/plugin/bundle.json").exists() {
        assert_eq!(checked.code, 2, "{}{}", checked.out, checked.err);
        assert!(checked.err.contains("component.bundled.missing") && checked.err.contains("fix:"), "{}", checked.err);
        return;
    }
    assert_eq!(checked.code, 0, "native and component builds agree: {}{}", checked.out, checked.err);
    let again = ink(&["build", "--check", "-C", dir.to_str().unwrap(), "-o", root.join("four").to_str().unwrap()]);
    assert_eq!(again.out, checked.out, "--check twice gives one deploy");

    // warnings stop a strict build before it writes anything
    let strict = ink(&["build", "--strict", "-C", dir.to_str().unwrap(), "-o", root.join("five").to_str().unwrap()]);
    assert_eq!(strict.code, 6, "{}{}", strict.out, strict.err);
    assert!(!root.join("five").exists(), "a strict build that stops wrote something");

    // the format was read from the vault marker, and the decision replays
    let why = ink(&["explain", "--why", "site.format", "-C", dir.to_str().unwrap()]);
    assert_eq!(why.code, 0, "{}{}", why.out, why.err);
    assert!(why.out.contains("obsidian"), "{}", why.out);
}
