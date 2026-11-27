//! A loaded component, adapted to the engine's own plugin traits: [`Component<Format>`]
//! implements [`engine::Format`] and [`Component<Theme>`] implements [`engine::Theme`], so
//! `Engine<B, P, F, T>` runs components with no change. Everything a call says goes to the
//! engine's record; what a call that has no record says (a theme's files, a format's choice of
//! file) is kept aside, for the caller to take with [`Component::aside`].

use std::cell::RefCell;

use engine::{BaseUrl, File, Html, Record, Said};
use model::{ContentPath, Diagnostic, Document, Front, Hash, Page, Read, Site, Subject};
use wasmtime::Store;
use wasmtime::component::{Linker, ResourceAny};

use crate::bind::format::{Format as Formats, FormatPre};
use crate::bind::plugin::ink::model::{read as front, site as sited};
use crate::bind::theme::{Theme as Themes, ThemePre};
use crate::cache::Origin;
use crate::capability::State;
use crate::error::{Error, Failure};
use crate::runtime::{Runtime, Shared};

/// A plugin loaded as a component of world `W`.
#[derive(Debug)]
pub struct Component<W> {
    shared: std::sync::Arc<Shared>,
    name: String,
    origin: Origin,
    world: W,
    aside: RefCell<Said>,
}

/// The `ink:format` world.
pub struct Format {
    pre: FormatPre<State>,
}

/// The `ink:theme` world, with the instance rendering the current site.
pub struct Theme {
    pre: ThemePre<State>,
    base: BaseUrl,
    /// The hash of the component's bytes.
    digest: Hash,
    session: RefCell<Option<Session>>,
}

/// One instance of a theme, rendering one site.
struct Session {
    store: Store<State>,
    bindings: Themes,
    renderer: ResourceAny,
    site: Site,
}

impl std::fmt::Debug for Format {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Format")
    }
}

impl std::fmt::Debug for Theme {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Theme").field("base", &self.base).finish_non_exhaustive()
    }
}

impl Runtime {
    /// The format plugin `name` from the component in `bytes`.
    pub fn format(&self, name: &str, bytes: &[u8]) -> Result<Component<Format>, Error> {
        let (compiled, origin) = self.shared.compile(name, bytes)?;
        let mut linker = Linker::new(&self.shared.engine);
        let link = |source| Error::Link { plugin: name.to_owned(), source };
        Formats::add_to_linker::<State, wasmtime::component::HasSelf<State>>(&mut linker, |state| state)
            .map_err(link)?;
        let pre = linker.instantiate_pre(&compiled).and_then(FormatPre::new).map_err(link)?;
        Ok(Component {
            shared: self.shared.clone(),
            name: name.to_owned(),
            origin,
            world: Format { pre },
            aside: RefCell::default(),
        })
    }

    /// The theme in `bytes`, for a site served at `base`; its name is the one it gives.
    pub fn theme(&self, bytes: &[u8], base: BaseUrl) -> Result<Component<Theme>, Error> {
        let (compiled, origin) = self.shared.compile("theme", bytes)?;
        let mut linker = Linker::new(&self.shared.engine);
        let link = |source| Error::Link { plugin: "theme".to_owned(), source };
        Themes::add_to_linker::<State, wasmtime::component::HasSelf<State>>(&mut linker, |state| state)
            .map_err(link)?;
        let pre = linker.instantiate_pre(&compiled).and_then(ThemePre::new).map_err(link)?;
        let mut store = self.shared.store("theme");
        let name = self
            .shared
            .call(&mut store, |store| pre.instantiate(&mut *store)?.call_name(store))
            .map_err(|failure| Error::Named { failure })?;
        let world = Theme { pre, base, digest: Hash::of(bytes), session: RefCell::new(None) };
        Ok(Component { shared: self.shared.clone(), name, origin, world, aside: RefCell::default() })
    }
}

impl<W> Component<W> {
    /// The plugin's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether it was compiled now or read from the cache.
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// Everything said by calls that had no record to say it into, taken.
    pub fn aside(&self) -> Said {
        self.aside.take()
    }

    /// Keeps `failure` aside.
    fn fail(&self, failure: &Failure) {
        self.aside.borrow_mut().diagnose(failure.diagnostic(&self.name));
    }
}

impl Component<Format> {
    /// One call, in an instance of its own: a plugin call shares nothing with the last.
    fn run<T>(
        &self,
        call: impl FnOnce(&Formats, &mut Store<State>) -> wasmtime::Result<T>,
    ) -> (Result<T, Failure>, Said) {
        let mut store = self.shared.store(&self.name);
        let pre = &self.world.pre;
        let result = self.shared.call(&mut store, |store| {
            let bindings = pre.instantiate(&mut *store)?;
            call(&bindings, store)
        });
        (result, store.into_data().said)
    }
}

impl engine::Format for Component<Format> {
    /// Whether the plugin reads `path`; a call that fails reads nothing and is kept aside.
    fn handles(&self, path: &ContentPath) -> bool {
        let (result, said) = self.run(|bindings, store| bindings.format().call_handles(store, &path.to_string()));
        self.aside.borrow_mut().diagnostics.extend(said.diagnostics);
        result.unwrap_or_else(|failure| {
            self.fail(&failure);
            false
        })
    }

    /// The file read; a call that fails, or a read the model refuses, is an empty read with the
    /// diagnostic saying why, about the file.
    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        let (result, said) = self.run(|bindings, store| bindings.format().call_read(store, &path.to_string(), bytes));
        let read =
            result.and_then(|read: front::Read| Read::try_from(read).map_err(|source| Failure::Malformed { source }));
        let mut read = read.unwrap_or_else(|failure| {
            let mut empty = Read::new(Document::default(), Front::default(), None);
            empty.diagnostics.push(failure.diagnostic(&self.name).on(Subject::File(path.clone())));
            empty
        });
        read.diagnostics.extend(said.diagnostics);
        read.notices.extend(said.notices);
        read
    }
}

impl Component<Theme> {
    /// Makes `slot` the session rendering `site`, unless it already is: a new site, or a session
    /// a failed call ended, gets a fresh instance. What a failed start said goes to `record`.
    fn start<R: Record + ?Sized>(
        &self,
        site: &Site,
        slot: &mut Option<Session>,
        record: &mut R,
    ) -> Result<(), Failure> {
        if slot.as_ref().is_some_and(|session| session.site == *site) {
            return Ok(());
        }
        *slot = None;
        let wire = sited::Site::try_from(site).map_err(|source| Failure::Unmirrored { source })?;
        let mut store = self.shared.store(&self.name);
        let (pre, base) = (&self.world.pre, self.world.base.as_ref());
        let started = self.shared.call(&mut store, |store| {
            let bindings = pre.instantiate(&mut *store)?;
            let renderer = bindings.pages().renderer().call_constructor(&mut *store, base, &wire)?;
            Ok((bindings, renderer))
        });
        let said = std::mem::take(&mut store.data_mut().said);
        Self::pass(said, record);
        let (bindings, renderer) = started?;
        *slot = Some(Session { store, bindings, renderer, site: site.clone() });
        Ok(())
    }

    /// Everything in `said`, into `record`, in order.
    fn pass<R: Record + ?Sized>(said: Said, record: &mut R) {
        said.diagnostics.into_iter().for_each(|diagnostic| record.diagnose(diagnostic));
        said.notices.into_iter().for_each(|notice| record.notice(notice));
    }

    /// One page through the session rendering `site`.
    fn page<R: Record + ?Sized>(
        &self,
        page: &Page,
        site: &Site,
        record: &mut R,
    ) -> Result<Result<String, Diagnostic>, Failure> {
        let mut slot = self.world.session.borrow_mut();
        self.start(site, &mut slot, record)?;
        let wire = sited::Page::try_from(page).map_err(|source| Failure::Unmirrored { source })?;
        let Some(Session { store, bindings, renderer, .. }) = slot.as_mut() else { return Err(Failure::Ended) };
        let rendered =
            self.shared.call(store, |store| bindings.pages().renderer().call_render(store, *renderer, &wire));
        Self::pass(std::mem::take(&mut store.data_mut().said), record);
        if rendered.is_err() {
            *slot = None;
        }
        match rendered? {
            Ok(html) => Ok(Ok(html)),
            Err(refused) => Diagnostic::try_from(refused).map(Err).map_err(|source| Failure::Malformed { source }),
        }
    }
}

impl engine::Theme for Component<Theme> {
    fn name(&self) -> &str {
        &self.name
    }

    /// The theme's files; a call that fails, or a file the model refuses, is kept aside.
    fn assets(&self) -> Vec<File> {
        let mut store = self.shared.store(&self.name);
        let pre = &self.world.pre;
        let result = self.shared.call(&mut store, |store| pre.instantiate(&mut *store)?.call_assets(store));
        Self::pass(store.into_data().said, &mut *self.aside.borrow_mut());
        let files = result.unwrap_or_else(|failure| {
            self.fail(&failure);
            Vec::new()
        });
        files
            .into_iter()
            .filter_map(|file| File::try_from(file).map_err(|source| self.fail(&Failure::Malformed { source })).ok())
            .collect()
    }

    /// The hash of the component's bytes and of the workspace's `Cargo.lock`: the host's
    /// highlighter is imported by the component and reaches its bytes, so its version is part of
    /// what the pages are made from. Its tokens join the hash when themes take them.
    fn identity(&self) -> Option<Hash> {
        const LOCK: &[u8] = include_bytes!("../../../Cargo.lock");
        Some(Hash::of([self.world.digest.as_bytes().as_slice(), LOCK].concat()))
    }

    /// The page as the component renders it; a call that fails is the theme's refusal, with
    /// the failure as its diagnostic, about the page's file.
    fn render<R: Record + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        match self.page(page, site, record) {
            Ok(rendered) => rendered.map(Html::from),
            Err(failure) => Err(failure.diagnostic(&self.name).on(Subject::File(page.file.clone()))),
        }
    }
}
