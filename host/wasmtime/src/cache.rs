//! Compiled components, kept as `.cwasm` files so a build pays Cranelift once per plugin
//! version (decision 0035). A file is keyed by the plugin's BLAKE3 digest and the engine's
//! [`precompile_compatibility_hash`](wasmtime::Engine::precompile_compatibility_hash), which
//! changes with wasmtime's version, its settings and the CPU features, and starts with the
//! BLAKE3 of the rest, so a torn or corrupted file is compiled again rather than loaded.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};

use model::Hash;
use wasmtime::component::Component;

use crate::error::Error;

/// A folder of compiled components.
#[derive(Clone, Debug)]
pub struct Cache {
    dir: PathBuf,
}

/// Where a loaded component came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Origin {
    /// Compiled now, and written to the cache when there is one.
    Compiled,
    /// Read from the cache.
    Cached,
    /// Compiled now because the cached file did not check (torn, corrupted, or refused by
    /// wasmtime), and written over it.
    Replaced,
}

impl Cache {
    /// The cache in `dir`, created when first written.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The file for `bytes` under `engine`.
    fn path(&self, engine: &wasmtime::Engine, bytes: &[u8]) -> PathBuf {
        let mut compatible = DefaultHasher::new();
        engine.precompile_compatibility_hash().hash(&mut compatible);
        self.dir.join(format!("{}-{:016x}.cwasm", Hash::of(bytes), compatible.finish()))
    }

    /// The component in `bytes`, from the cache when it holds it, else compiled and kept.
    pub(crate) fn load(
        &self,
        engine: &wasmtime::Engine,
        plugin: &str,
        bytes: &[u8],
    ) -> Result<(Component, Origin), Error> {
        let path = self.path(engine, bytes);
        let origin = match Self::read(engine, &path)? {
            Ok(component) => return Ok((component, Origin::Cached)),
            Err(origin) => origin,
        };
        let component = Compile(plugin).run(engine, bytes)?;
        let compiled = component.serialize().map_err(|source| Error::Compile { plugin: plugin.to_owned(), source })?;
        Self::write(&path, &compiled)?;
        Ok((component, origin))
    }

    #[expect(unsafe_code, reason = "loading a .cwasm is unsafe; decision 0035 allows this one place")]
    /// The cached component at `path`; otherwise how the one compiled instead will have come.
    fn read(engine: &wasmtime::Engine, path: &Path) -> Result<Result<Component, Origin>, Error> {
        let file = match std::fs::read(path) {
            Ok(file) => file,
            Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => return Ok(Err(Origin::Compiled)),
            Err(source) => return Err(Error::Cache { path: path.to_owned(), source }),
        };
        let Some((digest, compiled)) = file.split_first_chunk::<32>() else { return Ok(Err(Origin::Replaced)) };
        if Hash::of(compiled).as_bytes() != digest {
            return Ok(Err(Origin::Replaced));
        }
        // SAFETY: wasmtime runs a `.cwasm` as native code, so it must be one wasmtime wrote.
        // This one was: it is in our own cache, under a name that binds it to this engine's
        // compatibility hash, and its digest checked, so it is the bytes `write` put there.
        let component = unsafe { Component::deserialize(engine, compiled) };
        Ok(component.map_err(|_| Origin::Replaced))
    }

    /// Keeps `compiled` at `path`, written beside it first and renamed, so no reader sees half and
    /// the last of two writers wins whole.
    fn write(path: &Path, compiled: &[u8]) -> Result<(), Error> {
        let failed = |source| Error::Cache { path: path.to_owned(), source };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(failed)?;
        }
        // one partial file per process, so builds sharing the cache never write the same one
        let partial = path.with_extension(format!("{}.partial", std::process::id()));
        let mut file = Hash::of(compiled).as_bytes().to_vec();
        file.extend_from_slice(compiled);
        std::fs::write(&partial, file).map_err(failed)?;
        std::fs::rename(&partial, path).map_err(failed)
    }
}

/// Compiling one plugin's component with Cranelift.
pub(crate) struct Compile<'p>(pub(crate) &'p str);

impl Compile<'_> {
    pub(crate) fn run(&self, engine: &wasmtime::Engine, bytes: &[u8]) -> Result<Component, Error> {
        Component::new(engine, bytes).map_err(|source| Error::Compile { plugin: self.0.to_owned(), source })
    }
}
