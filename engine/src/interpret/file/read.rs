//! The stage reading files: each kept file a format handles is read into a document and its
//! frontmatter. The rest are plain files a page may use as is.

use model::{ContentPath, Diagnostic, FileFacts, Problem, Subject};

use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;
use crate::interpret::tree::InRoot;
use crate::{Blobs, Formats};

/// A file a format read: a page, if it is published.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub(crate) path: ContentPath,
    /// Its path below the root.
    pub(crate) relative: ContentPath,
    pub(crate) read: model::Read,
    pub(crate) facts: Option<FileFacts>,
}

impl Source {
    /// It, as a decision's subject.
    pub(crate) fn subject(&self) -> model::decision::File {
        model::decision::File(self.path.clone())
    }
}

/// The kept files, split into what formats read and the rest.
#[derive(Debug)]
pub(crate) struct Split<'a> {
    pub(crate) sources: Vec<Source>,
    pub(crate) files: Vec<InRoot<'a>>,
}

/// Reads each kept file through the first format that handles it.
#[derive(Debug)]
pub(crate) struct Read<'a, B, F> {
    pub(crate) blobs: &'a B,
    pub(crate) formats: &'a F,
}

impl<'s, B: Blobs, F: Formats> Stage<Vec<InRoot<'s>>> for Read<'_, B, F> {
    type Out = Split<'s>;

    /// A blob the store lacks is `source.blob.missing`. What the format says about a file is
    /// recorded as about that file.
    fn run(&self, kept: Vec<InRoot<'s>>, journal: &mut Journal) -> Split<'s> {
        let mut split = Split { sources: Vec::new(), files: Vec::new() };
        for file in kept {
            let path = &file.entry.path;
            if !self.formats.handles(path) {
                split.files.push(file);
                continue;
            }
            let subject = Subject::File(path.clone());
            // The hash is the only thing a `BlobMissing` holds, and the entry already names it.
            let Ok(bytes) = self.blobs.read(&file.entry.blob) else {
                let problem = Problem::SourceBlobMissing { path: path.clone() };
                journal.diagnose(Diagnostic::new(problem).on(subject));
                continue;
            };
            let Some(mut read) = self.formats.read(path, &bytes) else {
                split.files.push(file);
                continue;
            };
            for mut diagnostic in std::mem::take(&mut read.diagnostics) {
                diagnostic.subject.get_or_insert_with(|| subject.clone());
                journal.diagnose(diagnostic);
            }
            for mut notice in std::mem::take(&mut read.notices) {
                notice.subject.get_or_insert_with(|| subject.clone());
                journal.notice(notice);
            }
            let facts = file.facts.cloned();
            split.sources.push(Source { path: path.clone(), relative: file.relative.clone(), read, facts });
        }
        split
    }
}
