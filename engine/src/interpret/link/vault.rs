//! A vault's settings, as far as resolving links reads them: the attachment folder Obsidian
//! keeps in `.obsidian/app.json` under the root (`attachmentFolderPath`). Every other setting
//! there is Obsidian's own and is not read.

use std::fmt;

use model::{ContentPath, Diagnostic, PathError, Problem, Subject};
use serde::Deserialize;

use crate::Blobs;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;
use crate::interpret::tree::Scope;

/// Where Obsidian keeps the settings read, under the root.
const APP: &str = ".obsidian/app.json";

/// Where a vault keeps the files its notes embed, as `attachmentFolderPath` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Attachments {
    /// `/`: the vault's own folder.
    Root,
    /// `./`: the folder of the note that embeds the file.
    Beside,
    /// `./name`: a folder of that name in the note's folder.
    Below(ContentPath),
    /// `name`: one folder of the vault.
    Folder(ContentPath),
}

impl Attachments {
    /// Whether `file` is in the attachment folder of the page at `from`; both are snapshot paths,
    /// the folder is read under the root.
    pub(crate) fn holds(&self, scope: &Scope<'_>, from: &ContentPath, file: &ContentPath) -> bool {
        let below = |path: &ContentPath| scope.folder().map_or_else(|| Some(path.clone()), |root| path.strip(root));
        let folder = below(file).and_then(|file| file.parent());
        let page = below(from).and_then(|from| from.parent());
        match self {
            Self::Root => folder.is_none(),
            Self::Beside => folder == page,
            Self::Below(name) => folder == Some(page.map_or_else(|| name.clone(), |page| page.join(name))),
            Self::Folder(name) => folder.as_ref() == Some(name),
        }
    }
}

/// Why `attachmentFolderPath` names no folder.
#[derive(Debug)]
pub(crate) enum Unread {
    /// The file is not the JSON object Obsidian writes.
    Json(serde_json::Error),
    /// The setting is no path inside the vault.
    Folder {
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
            Self::Folder { written, .. } => {
                write!(out, "`attachmentFolderPath` is `{written}`, no folder of the vault")
            }
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

/// `.obsidian/app.json`, the keys read.
#[derive(Debug, Deserialize)]
struct App {
    #[serde(rename = "attachmentFolderPath")]
    attachments: Option<String>,
}

impl TryFrom<&[u8]> for Attachments {
    type Error = Unread;

    /// The folder `attachmentFolderPath` names; unset or empty, the vault's own folder, as
    /// Obsidian has it.
    fn try_from(bytes: &[u8]) -> Result<Self, Unread> {
        let app: App = serde_json::from_slice(bytes).map_err(Unread::Json)?;
        let written = app.attachments.unwrap_or_default();
        let folder = |name: &str| {
            name.trim_end_matches('/')
                .parse::<ContentPath>()
                .map_err(|source| Unread::Folder { written: written.clone(), source })
        };
        match written.trim() {
            "" | "/" => Ok(Self::Root),
            "." | "./" => Ok(Self::Beside),
            below if below.starts_with("./") => folder(below.trim_start_matches("./")).map(Self::Below),
            name => folder(name.trim_start_matches('/')).map(Self::Folder),
        }
    }
}

/// Reads a vault's settings from the snapshot, when the root holds them.
#[derive(Debug)]
pub(crate) struct Settings<'a, B> {
    pub(crate) blobs: &'a B,
}

impl<B: Blobs> Stage<&Scope<'_>> for Settings<'_, B> {
    /// The attachment folder, when the root holds `.obsidian/app.json` and it reads.
    type Out = Option<Attachments>;

    /// A file that does not read is `vault.settings.invalid`, and links resolve as if no
    /// attachment folder were set.
    fn run(&self, scope: &Scope<'_>, journal: &mut Journal) -> Option<Attachments> {
        let file = scope.inside.iter().find(|file| file.relative.as_str() == APP)?;
        let path = file.entry.path.clone();
        let subject = Subject::File(path.clone());
        let Ok(bytes) = self.blobs.read(&file.entry.blob) else {
            journal.diagnose(Diagnostic::new(Problem::SourceBlobMissing { path }).on(subject));
            return None;
        };
        match Attachments::try_from(bytes.as_ref()) {
            Ok(attachments) => Some(attachments),
            Err(unread) => {
                let problem = Problem::VaultSettingsInvalid { path, reason: unread.to_string() };
                journal.diagnose(Diagnostic::new(problem).on(subject));
                None
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Attachments, Unread};

    fn read(json: &str) -> Result<Attachments, Unread> {
        Attachments::try_from(json.as_bytes())
    }

    #[test]
    fn each_form_of_the_setting_names_its_folder() {
        assert_eq!(read("{}").unwrap(), Attachments::Root);
        assert_eq!(read(r#"{"attachmentFolderPath": "/"}"#).unwrap(), Attachments::Root);
        assert_eq!(read(r#"{"attachmentFolderPath": "./"}"#).unwrap(), Attachments::Beside);
        assert_eq!(
            read(r#"{"attachmentFolderPath": "./files"}"#).unwrap(),
            Attachments::Below("files".parse().unwrap())
        );
        assert_eq!(
            read(r#"{"attachmentFolderPath": "assets/img"}"#).unwrap(),
            Attachments::Folder("assets/img".parse().unwrap())
        );
    }

    #[test]
    fn a_setting_that_is_no_folder_is_refused() {
        assert!(matches!(read(r#"{"attachmentFolderPath": 3}"#), Err(Unread::Json(_))));
        assert!(matches!(read(r#"{"attachmentFolderPath": "../out"}"#), Err(Unread::Folder { .. })));
        assert!(matches!(read("not json"), Err(Unread::Json(_))));
    }
}
