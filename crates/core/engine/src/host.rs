//! What the engine asks of its host: bytes, pins, formats and a theme. Each is a trait the
//! [`Engine`](crate::Engine) is generic over; the CLI picks native types, tests pick in-memory
//! ones. The engine mirrors config's API in [`Pins`] so it builds and tests on its own. What a
//! plugin finds worth saying while it works goes into a [`Sink`] the engine gives it; the trait is
//! the model's, so a plugin reaches it without the engine.

use std::borrow::Cow;
use std::fmt;

use model::code::Code;
use model::{ContentPath, Diagnostic, File, Hash, Page, Read, Sink, Site, Span};

use crate::render::Reads;

/// Where the engine reads a snapshot's bytes from.
pub trait Blobs {
    /// The bytes whose hash is `hash`.
    fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing>;
}

impl<B: Blobs + ?Sized> Blobs for &B {
    fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing> {
        B::read(self, hash)
    }
}

/// A blob the store does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobMissing {
    /// The hash asked for.
    pub hash: Hash,
}

impl fmt::Display for BlobMissing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the blob store holds no blob {}", self.hash)
    }
}

impl std::error::Error for BlobMissing {}

/// The author's pins in `ink.toml`, typed by their code.
pub trait Pins: Patterns {
    /// The value pinned for `C` on `subject`, if `ink.toml` pins one.
    fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>>;
}

impl<P: Pins + ?Sized> Pins for &P {
    fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>> {
        P::pin::<C>(self, subject)
    }
}

/// The `include` and `exclude` patterns of `ink.toml`.
pub trait Patterns {
    /// Where the first `include` pattern matching `path` is written, if one does.
    fn included(&self, path: &ContentPath) -> Option<Span>;
    /// Where the first `exclude` pattern matching `path` is written, if one does.
    fn excluded(&self, path: &ContentPath) -> Option<Span>;
}

impl<P: Patterns + ?Sized> Patterns for &P {
    fn included(&self, path: &ContentPath) -> Option<Span> {
        P::included(self, path)
    }

    fn excluded(&self, path: &ContentPath) -> Option<Span> {
        P::excluded(self, path)
    }
}

/// A pinned value and where in `ink.toml` it is written. It becomes the decision, with
/// `Evidence::Config { span }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin<V> {
    /// The value.
    pub value: V,
    /// The whole `key = value`.
    pub span: Span,
}

/// A site with no `ink.toml`: nothing is pinned, nothing included or excluded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoPins;

impl Pins for NoPins {
    fn pin<C: Code>(&self, _subject: &C::Subject) -> Option<Pin<C::Value>> {
        None
    }
}

impl Patterns for NoPins {
    fn included(&self, _path: &ContentPath) -> Option<Span> {
        None
    }

    fn excluded(&self, _path: &ContentPath) -> Option<Span> {
        None
    }
}

/// A format plugin: reads one file's bytes into a document and its frontmatter.
pub trait Format {
    /// Whether this format reads the file at `path`.
    fn handles(&self, path: &ContentPath) -> bool;
    /// The file at `path`, read. It resolves nothing: links to other files stay as written.
    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read;
}

impl<F: Format + ?Sized> Format for &F {
    fn handles(&self, path: &ContentPath) -> bool {
        F::handles(self, path)
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        F::read(self, path, bytes)
    }
}

/// The formats of a build, asked in order: a tuple of [`Format`]s, so formats stack.
pub trait Formats {
    /// Whether any format reads the file at `path`.
    fn handles(&self, path: &ContentPath) -> bool;
    /// The file at `path`, read by the first format that handles it; `None` when none does.
    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Option<Read>;
}

/// Implements [`Formats`] for a tuple of [`Format`]s, the first that handles a file reading it.
/// The same impl over tuples of every length up to four, which a generic cannot write.
///
/// ```text
/// formats!(A 0, B 1);
/// // makes `impl<A: Format, B: Format> Formats for (A, B)`
/// ```
macro_rules! formats {
    ($($format:ident $at:tt),+) => {
        impl<$($format: Format),+> Formats for ($($format,)+) {
            fn handles(&self, path: &ContentPath) -> bool {
                false $(|| self.$at.handles(path))+
            }

            fn read(&self, path: &ContentPath, bytes: &[u8]) -> Option<Read> {
                $(
                    if self.$at.handles(path) {
                        return Some(self.$at.read(path, bytes));
                    }
                )+
                None
            }
        }
    };
}

formats!(A 0);
formats!(A 0, B 1);
formats!(A 0, B 1, C 2);
formats!(A 0, B 1, C 2, D 3);

/// A theme plugin: renders each page, and gives the files every page shares.
pub trait Theme {
    /// Its name, as diagnostics show it.
    fn name(&self) -> &str;
    /// CSS, fonts and images, asked for once per build.
    fn assets(&self) -> Vec<File>;
    /// One page's HTML. `site` is the whole site, for navigation and links. What rendering
    /// finds worth saying (math that does not parse, code in a language no grammar knows) goes
    /// into `record`, also when the page is refused.
    #[cfg_attr(
        target_pointer_width = "64",
        expect(clippy::result_large_err, reason = "the theme contract returns the model's diagnostic as is (0014)")
    )]
    fn render<R: Sink + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic>;

    /// A hash of everything about the theme that reaches the bytes of a page besides the page
    /// and what [`Theme::reads`] says: a component's own hash, the tokens it was given, its
    /// version. It keys the pages a cache keeps, so two themes that write different bytes must
    /// not share one. `None`, which is what a theme that says nothing gives, keeps no page.
    fn identity(&self) -> Option<Hash> {
        None
    }

    /// What rendering `page` reads of the site beyond the page itself. A theme that reads
    /// only what [`Reads::Linked`] lists, or also the other pages' fronts for a listing
    /// ([`Reads::Listed`]), is rendered again only when that changed; the default,
    /// [`Reads::Whole`], is rendered again when anything did.
    fn reads(&self, _page: &Page) -> Reads {
        Reads::Whole
    }
}

impl<T: Theme + ?Sized> Theme for &T {
    fn name(&self) -> &str {
        T::name(self)
    }

    fn assets(&self) -> Vec<File> {
        T::assets(self)
    }

    fn render<R: Sink + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic> {
        T::render(self, page, site, record)
    }

    fn identity(&self) -> Option<Hash> {
        T::identity(self)
    }

    fn reads(&self, page: &Page) -> Reads {
        T::reads(self, page)
    }
}

/// A page as a theme rendered it. The engine adds nothing inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Html(String);

impl From<String> for Html {
    fn from(html: String) -> Self {
        Self(html)
    }
}

impl AsRef<str> for Html {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Html> for Vec<u8> {
    fn from(html: Html) -> Self {
        html.0.into_bytes()
    }
}
