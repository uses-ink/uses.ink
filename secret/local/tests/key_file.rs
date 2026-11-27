//! The key file: made private, reused, and refused when it is not.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_secret_local as secret_local;

use secret::{Binding, Plain, Secret};
use secret_local::{Error, Local};

#[test]
fn makes_the_key_file_private_and_reuses_it() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("secret.key");
    let first = Local::open(&path).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 32);
    #[cfg(unix)]
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&path).unwrap().permissions()) & 0o777,
        0o600
    );
    let row = Binding::from("c-1");
    let sealed = pollster::block_on(first.seal(&Plain::from("token"), &row)).unwrap();
    let second = Local::open(&path).unwrap();
    assert_eq!(pollster::block_on(second.open(&sealed, &row)).unwrap().expose(), b"token");
}

#[test]
fn sits_beside_the_database() {
    let folder = tempfile::tempdir().unwrap();
    Local::beside(folder.path().join("ink.db")).unwrap();
    assert!(folder.path().join(Local::KEY).is_file());
}

#[test]
#[cfg(unix)]
fn refuses_a_key_others_can_read() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("secret.key");
    Local::open(&path).unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    let refused = Local::open(&path).unwrap_err();
    assert!(matches!(refused, Error::Permissions { mode: 0o644, .. }), "{refused}");
}

#[test]
fn refuses_a_key_of_the_wrong_length_and_a_symlink() {
    let folder = tempfile::tempdir().unwrap();
    let short = folder.path().join("short.key");
    std::fs::write(&short, b"short").unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&short, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    assert!(matches!(Local::open(&short).unwrap_err(), Error::Length { found: 5, .. }));
    #[cfg(unix)]
    {
        let link = folder.path().join("link.key");
        std::os::unix::fs::symlink(&short, &link).unwrap();
        assert!(matches!(Local::open(&link).unwrap_err(), Error::NotFile { .. }));
    }
}

#[test]
fn debug_shows_no_key() {
    assert_eq!(format!("{:?}", Local::ephemeral().unwrap()), "Local(<key redacted>)");
}
