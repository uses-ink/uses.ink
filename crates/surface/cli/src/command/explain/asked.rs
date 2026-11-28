//! One decision named on the command line, by an optional path and a code: what `ink explain
//! --why` replays and `ink pin` writes.

use model::{AnyCode, ContentPath, Snapshot, Subject};

use crate::output::error::{AskError, Error};

/// A decision code and the subject it is asked about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Asked {
    pub(crate) code: AnyCode,
    pub(crate) subject: Subject,
}

impl Asked {
    /// `code` on the file at `path`, or on the site without one. A code of the other scope is
    /// refused: a file's code needs its path, a site code takes none.
    pub(crate) fn read(path: Option<&str>, code: &str) -> Result<Self, Error> {
        let code: AnyCode = code.parse().map_err(|source| AskError::Code { source })?;
        let subject = match path {
            Some(written) => {
                let path: ContentPath =
                    written.parse().map_err(|source| AskError::Path { written: written.to_owned(), source })?;
                Subject::File(path)
            }
            None => Subject::Site,
        };
        if subject.scope() != code.scope() {
            return Err(AskError::Scope { code, subject }.into());
        }
        Ok(Self { code, subject })
    }

    /// The words `--why` was given: a code alone, or a path then a code.
    pub(crate) fn words(words: &[String]) -> Result<Self, Error> {
        match words {
            [code] => Self::read(None, code),
            [path, code] => Self::read(Some(path), code),
            _ => Err(AskError::Words { given: words.len() }.into()),
        }
    }

    /// Refuses a file the snapshot does not hold.
    pub(crate) fn within(&self, snapshot: &Snapshot) -> Result<(), Error> {
        match &self.subject {
            Subject::File(path) if snapshot.entry(path).is_none() => {
                Err(AskError::Unknown { path: path.clone() }.into())
            }
            _ => Ok(()),
        }
    }
}
