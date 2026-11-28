//! `ink:theme-quiet` as a wasm component: the quiet theme's native core
//! (`uses-ink-theme-quiet`) behind the `ink:theme` world, its code highlighted and its
//! diagnostics recorded through the host ([`sdk::Host`]). Built for `wasm32-unknown-unknown` and
//! wrapped with `wit-component` by `cargo xtask component` (decision 0035).

mod bind;

use engine::{BaseUrl, Theme};
use model::{Diagnostic, Page, Problem, Site};
use sdk::Host;
use sdk::bind::ink::host::diagnostic;
use sdk::bind::ink::model::{diagnostic as said, file, site as sited};

use bind::exports::pages::{Guest, GuestRenderer};

/// The theme, as the component exports it.
pub(crate) struct Quiet;

impl Quiet {
    const NAME: &'static str = "ink:theme-quiet";

    /// A value the host sent that the model refuses.
    fn malformed(reason: &impl std::error::Error) -> Diagnostic {
        Diagnostic::new(Problem::PluginValueMalformed { plugin: Self::NAME.to_owned(), reason: reason.to_string() })
    }
}

impl bind::Guest for Quiet {
    fn name() -> String {
        Self::NAME.to_owned()
    }

    /// The theme's files; one the boundary cannot carry is left out and said.
    fn assets() -> Vec<file::File> {
        let mut files = Vec::new();
        for asset in theme_quiet::Quiet::<Host>::files() {
            match file::File::try_from(asset) {
                Ok(file) => files.push(file),
                Err(unmirrored) => diagnostic::emit(&Host::carried(&Diagnostic::new(Problem::PluginValueUnmirrored {
                    reason: unmirrored.to_string(),
                }))),
            }
        }
        files
    }
}

impl Guest for Quiet {
    type Renderer = Renderer;
}

/// The theme over one site, or why the host's site or address was refused.
pub(crate) struct Renderer(Result<(theme_quiet::Quiet<Host>, Site), said::Diagnostic>);

impl GuestRenderer for Renderer {
    fn new(base: String, site: sited::Site) -> Self {
        let base = base.parse::<BaseUrl>().map_err(|refused| Host::carried(&Quiet::malformed(&refused)));
        let site = Site::try_from(site).map_err(|refused| Host::carried(&Quiet::malformed(&refused)));
        Self(base.and_then(|base| Ok((theme_quiet::Quiet::new(base, Host), site?))))
    }

    fn render(&self, page: sited::Page) -> Result<String, said::Diagnostic> {
        let (theme, site) = self.0.as_ref().map_err(Clone::clone)?;
        let page = Page::try_from(page).map_err(|refused| Host::carried(&Quiet::malformed(&refused)))?;
        theme
            .render(&page, site, &mut Host)
            .map(|html| html.as_ref().to_owned())
            .map_err(|refused| Host::carried(&refused))
    }
}
