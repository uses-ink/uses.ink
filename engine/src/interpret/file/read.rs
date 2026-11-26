//! Reading: each kept file a format handles is read into a document and its frontmatter. The
//! rest are files a page may use as is.

use model::{ContentPath, Diagnostic, Evidence, FileFacts, Problem, Read, Span, Subject};

use crate::interpret::file::name::Name;
use crate::interpret::file::scope::InRoot;
use crate::interpret::record::Record;
use crate::{Blobs, Format};

/// A file a format read: a page, if it is published.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub(crate) path: ContentPath,
    /// Its path below the root.
    pub(crate) relative: ContentPath,
    pub(crate) read: Read,
    pub(crate) facts: Option<FileFacts>,
}

impl Source {
    pub(crate) fn subject(&self) -> Subject {
        Subject::File(self.path.clone())
    }

    pub(crate) fn name(&self) -> Name<'_> {
        Name::from(&self.relative)
    }

    pub(crate) fn frontmatter(&self, span: Span) -> Evidence {
        Evidence::Frontmatter { path: self.path.clone(), span }
    }

    pub(crate) fn filename(&self) -> Evidence {
        Evidence::Filename { path: self.path.clone() }
    }
}

/// The kept files, split into what formats read and the rest.
#[derive(Debug)]
pub(crate) struct Split<'a> {
    pub(crate) sources: Vec<Source>,
    pub(crate) files: Vec<InRoot<'a>>,
}

/// Each file goes to the first format that handles it. A blob the store lacks is
/// `source.blob.missing`.
pub(crate) fn read<'a>(
    kept: Vec<InRoot<'a>>,
    blobs: &impl Blobs,
    formats: &[&dyn Format],
    record: &mut Record,
) -> Split<'a> {
    let mut split = Split { sources: Vec::new(), files: Vec::new() };
    for file in kept {
        let Some(format) = formats.iter().find(|format| format.handles(file.path())) else {
            split.files.push(file);
            continue;
        };
        let bytes = match blobs.read(&file.entry.blob) {
            Ok(bytes) => bytes,
            Err(_missing) => {
                // The hash is the only thing a `BlobMissing` holds, and the entry already names it.
                let problem = Problem::SourceBlobMissing { path: file.path().clone() };
                record.diagnose(Diagnostic::new(problem).on(file.subject()));
                continue;
            }
        };
        let mut read = format.read(file.path(), &bytes);
        for mut diagnostic in std::mem::take(&mut read.diagnostics) {
            diagnostic.subject.get_or_insert_with(|| file.subject());
            record.diagnose(diagnostic);
        }
        for mut notice in std::mem::take(&mut read.notices) {
            notice.subject.get_or_insert_with(|| file.subject());
            record.notice(notice);
        }
        let facts = file.facts.cloned();
        split.sources.push(Source { path: file.path().clone(), relative: file.relative.clone(), read, facts });
    }
    split
}
