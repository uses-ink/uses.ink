//! `ink build --check`: the site built twice, through the native plugins and through the same
//! plugins as wasm components (decision 0035), refused when the deploys differ. Nothing is
//! written; a difference is `build.check.mismatch`, a content failure (exit 1).

use std::path::Path;

use engine::Said;
use host_wasmtime::{Budget, Cache, Component, Runtime};
use model::{DeployId, Diagnostic, Manifest, Problem};
use serde::Serialize;

use crate::args::Build;
use crate::cache::Caching;
use crate::error::Error;
use crate::exit::Exit;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::{Address, Source};

/// A build compared with its components' build.
#[derive(Debug)]
pub(crate) struct Compare<'a> {
    pub(crate) build: &'a Build,
    pub(crate) components: &'a Path,
}

/// What `ink build --check --json` prints.
#[derive(Debug, Serialize)]
struct Compared {
    schema: &'static str,
    /// The deploy the native plugins built, when the site rendered.
    native: Option<DeployId>,
    /// The deploy the components built.
    component: Option<DeployId>,
    errors: usize,
    warnings: usize,
}

/// The components' files in the folder `--components` names.
#[derive(Clone, Copy, Debug)]
enum Plugin {
    Format,
    Theme,
}

impl Plugin {
    const fn file(self) -> &'static str {
        match self {
            Self::Format => "format-markdown.wasm",
            Self::Theme => "theme-quiet.wasm",
        }
    }

    fn bytes(self, dir: &Path) -> Result<Vec<u8>, Error> {
        let path = dir.join(self.file());
        std::fs::read(&path).map_err(|source| Error::Component { path, source })
    }
}

impl Compare<'_> {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let address = Address::from_flag(self.build.base.as_deref())?;
        let loaded = Source::new(&self.build.dir, &self.build.origin).load()?;
        let caching = Caching::new(&self.build.dir, self.build.cache.as_deref());
        let images = caching.pixels();
        let native = loaded.render(&address, &images);

        let runtime =
            Runtime::new(Budget::default(), Some(Cache::new(self.components.join("cache")))).map_err(Self::plugin)?;
        let format =
            runtime.format("ink:format-markdown", &Plugin::Format.bytes(self.components)?).map_err(Self::plugin)?;
        let theme =
            runtime.theme(&Plugin::Theme.bytes(self.components)?, address.base.clone()).map_err(Self::plugin)?;
        let through = Source::new(&self.build.dir, &self.build.origin).interpret((&format,))?;
        let built = through.through(&address, &theme, &images);

        // the native build's diagnostics, then what only the components said
        let mut diagnostics = native.diagnostics.clone();
        let mut notices = native.notices.clone();
        notices.extend(caching.finish());
        for aside in [Self::aside(&format), Self::aside(&theme)] {
            diagnostics.extend(aside.diagnostics);
            notices.extend(aside.notices);
        }
        let (ours, theirs) = (native.deploy.as_ref().map(Manifest::id), built.deploy.as_ref().map(Manifest::id));
        if ours != theirs {
            let shown = |deploy: Option<DeployId>| deploy.map_or_else(|| "none".to_owned(), |id| id.to_string());
            let problem = Problem::BuildCheckMismatch { native: shown(ours), component: shown(theirs) };
            diagnostics.push(Diagnostic::new(problem));
        }
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let tally = io.said(&diagnostics, &notices, texts)?;
        if let Some(deploy) = ours {
            io.line(deploy)?;
        }
        io.json(&Compared {
            schema: "uses.build-check/v1",
            native: ours,
            component: theirs,
            errors: tally.errors,
            warnings: tally.warnings,
        })?;
        Ok(tally.exit(Strict::from(self.build.strict)))
    }

    fn plugin(source: host_wasmtime::Error) -> Error {
        Error::Plugin { source }
    }

    fn aside<W>(component: &Component<W>) -> Said {
        component.aside()
    }
}
