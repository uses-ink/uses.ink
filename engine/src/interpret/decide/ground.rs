//! What a ground offers witnesses, so one witness serves every ground that has it: a file's
//! paths and facts, or a page's read text.

use model::{ContentPath, Evidence, FileFacts, Span};

use crate::interpret::pipeline::file::name::Name;
use crate::interpret::pipeline::file::read::Source;

/// A ground about one file.
pub(crate) trait Filed {
    /// Its path in the snapshot.
    fn path(&self) -> &ContentPath;
    /// Its path below the root: `posts/a.md` under root `content` for `content/posts/a.md`.
    fn relative(&self) -> &ContentPath;
    /// What the source knows about it beyond its bytes.
    fn facts(&self) -> Option<&FileFacts>;

    /// Its name below the root, read for what it says.
    fn name(&self) -> Name<'_> {
        Name::from(self.relative())
    }

    /// The evidence of its name.
    fn named(&self) -> Evidence {
        Evidence::Filename { path: self.path().clone() }
    }
}

/// A ground holding a page a format read: the file decided about, or the site's home.
pub(crate) trait Sourced {
    /// The page, if there is one.
    fn source(&self) -> Option<&Source>;
}

impl Source {
    /// The evidence of a frontmatter value at `span`.
    pub(crate) fn front(&self, span: Span) -> Evidence {
        Evidence::Frontmatter { path: self.path.clone(), span }
    }
}

impl Filed for Source {
    fn path(&self) -> &ContentPath {
        &self.path
    }

    fn relative(&self) -> &ContentPath {
        &self.relative
    }

    fn facts(&self) -> Option<&FileFacts> {
        self.facts.as_ref()
    }
}

impl Sourced for Source {
    fn source(&self) -> Option<&Source> {
        Some(self)
    }
}
