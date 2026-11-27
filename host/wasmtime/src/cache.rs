//! Compiled components, kept as `.cwasm` files so a build pays Cranelift once per plugin
//! version (decision 0035). A file is keyed by the plugin's BLAKE3 digest and the engine's
//! [`precompile_compatibility_hash`](wasmtime::Engine::precompile_compatibility_hash), which
//! changes with wasmtime's version, its settings and the CPU features. A file starts with a
//! [`Seal`]: the BLAKE3 of the source component's digest, the engine's compatibility hash and the
//! compiled bytes, so a torn or corrupted file, or one copied from another plugin or engine, is
//! compiled again rather than loaded. The seal is not a signature: whoever can write the folder
//! can seal any bytes, so the folder is trusted as code the builder runs (see `Cache::read`).

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

    /// What binds an entry to the component `bytes` and to `engine`.
    fn seal(engine: &wasmtime::Engine, bytes: &[u8]) -> Seal {
        let mut compatible = DefaultHasher::new();
        engine.precompile_compatibility_hash().hash(&mut compatible);
        Seal { source: Hash::of(bytes), engine: compatible.finish() }
    }

    /// The component in `bytes`, from the cache when it holds it, else compiled and kept.
    pub(crate) fn load(
        &self,
        engine: &wasmtime::Engine,
        plugin: &str,
        bytes: &[u8],
    ) -> Result<(Component, Origin), Error> {
        let seal = Self::seal(engine, bytes);
        let path = self.dir.join(seal.file());
        let origin = match Self::read(engine, &seal, &path)? {
            Ok(component) => return Ok((component, Origin::Cached)),
            Err(origin) => origin,
        };
        let component = Compile(plugin).run(engine, bytes)?;
        let compiled = component.serialize().map_err(|source| Error::Compile { plugin: plugin.to_owned(), source })?;
        Self::write(&path, &seal, &compiled)?;
        Ok((component, origin))
    }

    /// The cached component at `path`; otherwise how the one compiled instead will have come.
    #[expect(unsafe_code, reason = "loading a .cwasm is unsafe; decision 0035 allows this one place")]
    fn read(engine: &wasmtime::Engine, seal: &Seal, path: &Path) -> Result<Result<Component, Origin>, Error> {
        let file = match std::fs::read(path) {
            Ok(file) => file,
            Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => return Ok(Err(Origin::Compiled)),
            Err(source) => return Err(Error::Cache { path: path.to_owned(), source }),
        };
        let Some((digest, compiled)) = file.split_first_chunk::<32>() else { return Ok(Err(Origin::Replaced)) };
        if &seal.over(compiled) != digest {
            return Ok(Err(Origin::Replaced));
        }
        // SAFETY: wasmtime runs a `.cwasm` as native code, so it must be one wasmtime wrote for
        // this component and this engine. The seal checks that the file was written for this
        // source digest and engine compatibility hash and that it is whole, which is all that
        // guards against a torn, corrupted or misplaced file. It does not guard against someone
        // who can write the cache folder: they can seal bytes of their own, and `deserialize`
        // would run them. The cache folder is therefore trusted exactly as the builder's own
        // binaries are, and must not be shared across trust boundaries.
        let component = unsafe { Component::deserialize(engine, compiled) };
        Ok(component.map_err(|_| Origin::Replaced))
    }

    /// Keeps `compiled` at `path`, written beside it first and renamed, so no reader sees half and
    /// the last of two writers wins whole.
    fn write(path: &Path, seal: &Seal, compiled: &[u8]) -> Result<(), Error> {
        let failed = |source| Error::Cache { path: path.to_owned(), source };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(failed)?;
        }
        // one partial file per process, so builds sharing the cache never write the same one
        let partial = path.with_extension(format!("{}.partial", std::process::id()));
        let mut file = seal.over(compiled).to_vec();
        file.extend_from_slice(compiled);
        std::fs::write(&partial, file).map_err(failed)?;
        std::fs::rename(&partial, path).map_err(failed)
    }
}

/// What an entry is bound to: the digest of the source component and the engine's compatibility
/// hash. Both come from what the builder holds, never from the file.
struct Seal {
    source: Hash,
    engine: u64,
}

impl Seal {
    /// The entry's file name.
    fn file(&self) -> String {
        format!("{}-{:016x}.cwasm", self.source, self.engine)
    }

    /// The seal over `compiled`: BLAKE3 of the source digest, the engine hash and the bytes.
    fn over(&self, compiled: &[u8]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.source.as_bytes());
        hasher.update(&self.engine.to_le_bytes());
        hasher.update(compiled);
        *hasher.finalize().as_bytes()
    }
}

/// Compiling one plugin's component with Cranelift.
pub(crate) struct Compile<'p>(pub(crate) &'p str);

impl Compile<'_> {
    pub(crate) fn run(&self, engine: &wasmtime::Engine, bytes: &[u8]) -> Result<Component, Error> {
        Component::new(engine, bytes).map_err(|source| Error::Compile { plugin: self.0.to_owned(), source })
    }
}
