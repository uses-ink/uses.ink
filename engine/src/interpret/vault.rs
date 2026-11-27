//! A vault's settings, as far as the engine reads them. Obsidian keeps them in `.obsidian/` under
//! the root: the attachment folder in `app.json` (resolving links), the templates folder in
//! `templates.json` and the core plugins that are on in `core-plugins.json` (which files are
//! never pages). Every other setting is Obsidian's own and is not read.
//!
//! A file that does not read is `vault.settings.invalid`, and the vault is read as if it had not
//! that file.

mod attachment;
mod template;

use std::fmt;

use model::{ContentPath, Diagnostic, PathError, Problem, Subject};

pub(crate) use attachment::Attachments;
pub(crate) use template::Templates;

use crate::Blobs;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;
use crate::interpret::tree::Scope;

/// Why a settings file reads no further.
#[derive(Debug)]
pub(crate) enum Unread {
    /// The file is not the JSON Obsidian writes.
    Json(serde_json::Error),
    /// A setting is no path inside the vault.
    Folder {
        /// The setting's key.
        key: &'static str,
        /// The setting, as written.
        written: String,
        /// Why it is no path.
        source: PathError,
    },
}

impl fmt::Display for Unread {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(source) => write!(out, "{source}"),
            Self::Folder { key, written, .. } => write!(out, "`{key}` is `{written}`, no folder of the vault"),
        }
    }
}

impl std::error::Error for Unread {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            Self::Folder { source, .. } => Some(source),
        }
    }
}

/// A folder setting as Obsidian writes it: below the vault, no leading or trailing `/`. `None`
/// when it is empty, which Obsidian takes as unset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Written<'a> {
    pub(crate) key: &'static str,
    pub(crate) text: &'a str,
}

impl Written<'_> {
    /// The folder it names.
    pub(crate) fn folder(self) -> Result<Option<ContentPath>, Unread> {
        let name = self.text.trim().trim_matches('/');
        if name.is_empty() {
            return Ok(None);
        }
        name.parse().map(Some).map_err(|source| Unread::Folder { key: self.key, written: self.text.to_owned(), source })
    }
}

/// A settings file of `.obsidian/`, read from its bytes.
pub(crate) trait Setting: for<'b> TryFrom<&'b [u8], Error = Unread> {
    /// Where it is, under the root.
    const FILE: &'static str;
}

/// What a vault's settings say that the engine uses.
#[derive(Debug, Default)]
pub(crate) struct Vault {
    /// Where the files notes embed are kept, when `app.json` says.
    pub(crate) attachments: Option<Attachments>,
    /// Which folder holds the vault's templates.
    pub(crate) templates: Templates,
}

/// Reads a vault's settings from the snapshot, those the root holds.
#[derive(Debug)]
pub(crate) struct Settings<'a, B> {
    pub(crate) blobs: &'a B,
}

impl<B: Blobs> Settings<'_, B> {
    /// The setting file `S` of the root, when it holds one and it reads; what does not is
    /// `vault.settings.invalid`.
    fn read<S: Setting>(&self, scope: &Scope<'_>, journal: &mut Journal) -> Option<S> {
        let file = scope.inside.iter().find(|file| file.relative.as_str() == S::FILE)?;
        let path = file.entry.path.clone();
        let subject = Subject::File(path.clone());
        let Ok(bytes) = self.blobs.read(&file.entry.blob) else {
            journal.diagnose(Diagnostic::new(Problem::SourceBlobMissing { path }).on(subject));
            return None;
        };
        match S::try_from(bytes.as_ref()) {
            Ok(setting) => Some(setting),
            Err(unread) => {
                let problem = Problem::VaultSettingsInvalid { path, reason: unread.to_string() };
                journal.diagnose(Diagnostic::new(problem).on(subject));
                None
            }
        }
    }
}

impl<B: Blobs> Stage<&Scope<'_>> for Settings<'_, B> {
    type Out = Vault;

    fn run(&self, scope: &Scope<'_>, journal: &mut Journal) -> Vault {
        let attachments = self.read::<Attachments>(scope, journal);
        let folder = self.read::<template::Folder>(scope, journal);
        let plugins = self.read::<template::Plugins>(scope, journal);
        Vault { attachments, templates: Templates::from((folder, plugins)) }
    }
}

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    use super::template::{Folder, Plugins};
    use super::{Attachments, Setting, Unread};

    /// The pieces of a settings file: JSON syntax, the keys read, and the values they take.
    const PIECES: &[&str] = &[
        "{",
        "}",
        "[",
        "]",
        ",",
        ":",
        "\"folder\"",
        "\"attachmentFolderPath\"",
        "\"templates\"",
        "true",
        "false",
        "null",
        "3",
        "\"\"",
        "\"/\"",
        "\"./\"",
        "\"./files\"",
        "\"Meta/Tpl/\"",
        "\"../out\"",
        "\"a//b\"",
        "\"\\u0000\"",
        "\"é\"",
        " ",
    ];

    /// Reads `bytes` as setting `S`: it reads, or is refused with a cause, and does so again the
    /// same way.
    fn reads<S: Setting + std::fmt::Debug>(bytes: &[u8]) -> Result<(), TestCaseError> {
        let first =
            S::try_from(bytes).map(|setting| format!("{setting:?}")).map_err(|unread: Unread| unread.to_string());
        let again =
            S::try_from(bytes).map(|setting| format!("{setting:?}")).map_err(|unread: Unread| unread.to_string());
        prop_assert_eq!(first, again);
        Ok(())
    }

    proptest! {
        // Seeded, so a failure reproduces; proptest prints the failing input.
        #![proptest_config(ProptestConfig {
            rng_seed: proptest::test_runner::RngSeed::Fixed(0x5e77_1a95),
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        #[test]
        fn bytes_never_panic_the_settings_readers(bytes in prop::collection::vec(any::<u8>(), 0..200)) {
            reads::<Attachments>(&bytes)?;
            reads::<Folder>(&bytes)?;
            reads::<Plugins>(&bytes)?;
        }

        #[test]
        fn json_ish_text_never_panics_the_settings_readers(pieces in prop::collection::vec(prop::sample::select(PIECES), 0..30)) {
            let text = pieces.concat();
            reads::<Attachments>(text.as_bytes())?;
            reads::<Folder>(text.as_bytes())?;
            reads::<Plugins>(text.as_bytes())?;
        }
    }
}
