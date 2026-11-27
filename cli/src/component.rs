//! `ink build --check`: the site built twice, through the native plugins and through the same
//! plugins as wasm components (decision 0035), refused when the deploys differ. The components
//! are the ones `ink` carries (decision 0054) unless `--components` names a folder. Nothing is
//! written; a difference is `build.check.mismatch`, a content failure (exit 1).

use engine::{Host, Said};
use host_wasmtime::{Budget, Cache, Component, Runtime};
use model::{DeployId, Diagnostic, Manifest, Problem};
use serde::Serialize;

use crate::args::Build;
use crate::bundle::{Components, Used};
use crate::cache::Caching;
use crate::error::Error;
use crate::exit::Exit;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::{Address, Source};
use crate::term::Answer;

/// A build compared with its components' build.
#[derive(Debug)]
pub(crate) struct Compare<'a> {
    pub(crate) build: &'a Build,
    pub(crate) components: Components,
}

/// What `ink build --check --json` prints.
#[derive(Debug, Serialize)]
struct Compared {
    schema: &'static str,
    /// The deploy the native plugins built, when the site rendered.
    native: Option<DeployId>,
    /// The deploy the components built.
    component: Option<DeployId>,
    /// The components that ran and where each came from.
    components: Vec<Used>,
    errors: usize,
    warnings: usize,
}

impl Compare<'_> {
    /// The theme the components build with.
    const THEME: &'static str = "ink:theme-quiet";

    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let address = Address::from_flag(self.build.base.as_deref())?;
        let loaded = Source::new(&self.build.dir, &self.build.origin).load()?;
        let caching = Caching::new(&self.build.dir, self.build.cache.as_deref());
        let images = caching.pixels(loaded.config.image());
        let native = loaded.render(&address, Host::new(&images));

        let runtime = Runtime::new(Budget::default(), Some(Cache::new(caching.root().join("components"))))
            .map_err(Self::plugin)?;
        let chosen = loaded.native;
        let (held_format, held_theme) = (self.components.hold(chosen.id())?, self.components.hold(Self::THEME)?);
        let format = runtime.format(chosen.id(), held_format.bytes()).map_err(Self::plugin)?;
        let theme = runtime.theme(Self::THEME, held_theme.bytes(), address.base.clone()).map_err(Self::plugin)?;
        let through = Source::new(&self.build.dir, &self.build.origin).interpret(|_| (&format,))?;
        let built = through.through(&address, &theme, Host::new(&images));

        // the native build's diagnostics, then what only the components said
        let mut diagnostics = native.diagnostics.clone();
        let mut notices = native.notices.clone();
        notices.extend(caching.finish());
        notices.extend([held_format.notice(), held_theme.notice()]);
        for aside in [Self::aside(&format), Self::aside(&theme)] {
            diagnostics.extend(aside.diagnostics);
            notices.extend(aside.notices);
        }
        let (ours, theirs) = (native.deploy.as_ref().map(Manifest::id), built.deploy.as_ref().map(Manifest::id));
        diagnostics.extend(Self::mismatch(ours, theirs));
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let tally = io.said(&diagnostics, &notices, texts)?;
        if let Some(deploy) = ours {
            io.result(&Answer::of(deploy))?;
        }
        io.json(&Compared {
            schema: "uses.build-check/v1",
            native: ours,
            component: theirs,
            components: vec![held_format.used(), held_theme.used()],
            errors: tally.errors,
            warnings: tally.warnings,
        })?;
        Ok(tally.exit(Strict::from(self.build.strict)))
    }

    /// The refusal when the two builds' deploys differ.
    fn mismatch(native: Option<DeployId>, component: Option<DeployId>) -> Option<Diagnostic> {
        if native == component {
            return None;
        }
        let shown = |deploy: Option<DeployId>| deploy.map_or_else(|| "none".to_owned(), |id| id.to_string());
        Some(Diagnostic::new(Problem::BuildCheckMismatch { native: shown(native), component: shown(component) }))
    }

    fn plugin(source: host_wasmtime::Error) -> Error {
        Error::Plugin { source }
    }

    fn aside<W>(component: &Component<W>) -> Said {
        component.aside()
    }
}
