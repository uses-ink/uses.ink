//! Hands the crate the hash of its own source, as `FARM_SOURCE_HASH`, for `farm --version`.

use std::io::Write as _;
use std::path::PathBuf;

#[path = "src/digest.rs"]
mod digest;

fn main() -> std::io::Result<()> {
    let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let hash = digest::Digest::of(&dir)?;
    let mut out = std::io::stdout().lock();
    writeln!(out, "cargo:rerun-if-changed={}", dir.display())?;
    writeln!(out, "cargo:rustc-env=FARM_SOURCE_HASH={hash}")
}
