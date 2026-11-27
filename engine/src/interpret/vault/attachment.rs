//! The attachment folder Obsidian keeps in `.obsidian/app.json` (`attachmentFolderPath`), which
//! resolving links reads. Every other setting there is Obsidian's own and is not read.

use model::ContentPath;
use serde::Deserialize;

use super::{Setting, Unread};
use crate::interpret::tree::Scope;

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

/// `.obsidian/app.json`, the keys read.
#[derive(Debug, Deserialize)]
struct App {
    #[serde(rename = "attachmentFolderPath")]
    attachments: Option<String>,
}

impl Setting for Attachments {
    const FILE: &'static str = ".obsidian/app.json";
}

impl TryFrom<&[u8]> for Attachments {
    type Error = Unread;

    /// The folder `attachmentFolderPath` names; unset or empty, the vault's own folder, as
    /// Obsidian has it.
    fn try_from(bytes: &[u8]) -> Result<Self, Unread> {
        let app: App = serde_json::from_slice(bytes).map_err(Unread::Json)?;
        let written = app.attachments.unwrap_or_default();
        let folder = |name: &str| {
            name.trim_end_matches('/').parse::<ContentPath>().map_err(|source| Unread::Folder {
                key: "attachmentFolderPath",
                written: written.clone(),
                source,
            })
        };
        match written.trim() {
            "" | "/" => Ok(Self::Root),
            "." | "./" => Ok(Self::Beside),
            below if below.starts_with("./") => folder(below.trim_start_matches("./")).map(Self::Below),
            name => folder(name.trim_start_matches('/')).map(Self::Folder),
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
