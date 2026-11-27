//! A vault's templates folder: `folder` in `.obsidian/templates.json`, when the templates core
//! plugin is on in `.obsidian/core-plugins.json`. Files in it are templates, never pages.

use std::collections::BTreeMap;

use model::ContentPath;
use serde::Deserialize;

use super::{Setting, Unread, Written};

/// Where a vault keeps its templates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Templates {
    /// No folder is set, or the plugin that reads it is off: the conventional `templates`,
    /// at the top of the root, in any case.
    #[default]
    Conventional,
    /// The folder `templates.json` names, below the root, in any case.
    Folder(ContentPath),
}

impl Templates {
    /// The conventional folder's name.
    const NAME: &'static str = "templates";

    /// How many leading segments of `relative`, a path below the root, are the templates folder,
    /// when the file is inside it.
    pub(crate) fn holds(&self, relative: &ContentPath) -> Option<usize> {
        let depth = match self {
            Self::Conventional => 1,
            Self::Folder(folder) => folder.segments().count(),
        };
        let folder: Vec<&str> = match self {
            Self::Conventional => vec![Self::NAME],
            Self::Folder(folder) => folder.segments().collect(),
        };
        let inside = relative.segments().count() > depth;
        let same = relative.segments().zip(&folder).all(|(segment, name)| segment.eq_ignore_ascii_case(name));
        (inside && same).then_some(depth)
    }
}

impl From<(Option<Folder>, Option<Plugins>)> for Templates {
    /// The folder, unless the templates plugin is off, which makes it stale.
    fn from((folder, plugins): (Option<Folder>, Option<Plugins>)) -> Self {
        let on = plugins.is_none_or(|plugins| plugins.has("templates"));
        match folder {
            Some(Folder(Some(folder))) if on => Self::Folder(folder),
            _ => Self::Conventional,
        }
    }
}

/// `.obsidian/templates.json`: the folder it names, or none.
#[derive(Debug)]
pub(crate) struct Folder(Option<ContentPath>);

/// The keys of `templates.json` that are read.
#[derive(Debug, Deserialize)]
struct Config {
    folder: Option<String>,
}

impl Setting for Folder {
    const FILE: &'static str = ".obsidian/templates.json";
}

impl TryFrom<&[u8]> for Folder {
    type Error = Unread;

    fn try_from(bytes: &[u8]) -> Result<Self, Unread> {
        let config: Config = serde_json::from_slice(bytes).map_err(Unread::Json)?;
        let text = config.folder.unwrap_or_default();
        Written { key: "folder", text: &text }.folder().map(Self)
    }
}

/// `.obsidian/core-plugins.json`: which core plugins are on. Obsidian wrote a list of the plugins
/// that are on, and now writes a map of each plugin to whether it is.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Plugins {
    List(Vec<String>),
    Map(BTreeMap<String, bool>),
}

impl Plugins {
    /// Whether the core plugin `id` is on.
    pub(crate) fn has(&self, id: &str) -> bool {
        match self {
            Self::List(on) => on.iter().any(|plugin| plugin == id),
            Self::Map(plugins) => plugins.get(id).copied().unwrap_or(false),
        }
    }
}

impl Setting for Plugins {
    const FILE: &'static str = ".obsidian/core-plugins.json";
}

impl TryFrom<&[u8]> for Plugins {
    type Error = Unread;

    fn try_from(bytes: &[u8]) -> Result<Self, Unread> {
        serde_json::from_slice(bytes).map_err(Unread::Json)
    }
}

#[cfg(test)]
mod test {
    use model::ContentPath;

    use super::{Folder, Plugins, Templates, Unread};

    fn path(text: &str) -> ContentPath {
        text.parse().unwrap()
    }

    fn folder(json: &str) -> Result<Option<ContentPath>, Unread> {
        Folder::try_from(json.as_bytes()).map(|folder| folder.0)
    }

    #[test]
    fn the_folder_is_the_one_templates_json_names() {
        assert_eq!(folder(r#"{"folder": "Meta/Tpl/"}"#).unwrap(), Some(path("Meta/Tpl")));
        assert_eq!(folder(r#"{"folder": "/Tpl"}"#).unwrap(), Some(path("Tpl")));
        assert_eq!(folder(r#"{"folder": ""}"#).unwrap(), None);
        assert_eq!(folder("{}").unwrap(), None);
    }

    #[test]
    fn a_file_that_names_no_folder_is_refused() {
        assert!(matches!(folder(r#"{"folder": "../out"}"#), Err(Unread::Folder { .. })));
        assert!(matches!(folder(r#"{"folder": 3}"#), Err(Unread::Json(_))));
        assert!(matches!(folder("[1]"), Err(Unread::Json(_))));
    }

    #[test]
    fn a_core_plugin_is_on_in_either_form_of_the_file() {
        let read = |json: &str| Plugins::try_from(json.as_bytes()).unwrap();
        assert!(read(r#"["file-explorer", "templates"]"#).has("templates"));
        assert!(!read(r#"["file-explorer"]"#).has("templates"));
        assert!(read(r#"{"templates": true}"#).has("templates"));
        assert!(!read(r#"{"templates": false}"#).has("templates"));
        assert!(!read("{}").has("templates"));
        assert!(Plugins::try_from(b"3".as_slice()).is_err());
    }

    #[test]
    fn a_stale_folder_is_not_used() {
        let tpl = || Some(Folder(Some(path("Tpl"))));
        let plugins = |json: &str| Some(Plugins::try_from(json.as_bytes()).unwrap());
        assert_eq!(Templates::from((tpl(), None)), Templates::Folder(path("Tpl")));
        assert_eq!(Templates::from((tpl(), plugins(r#"["templates"]"#))), Templates::Folder(path("Tpl")));
        assert_eq!(Templates::from((tpl(), plugins(r#"["file-explorer"]"#))), Templates::Conventional);
        assert_eq!(Templates::from((None, None)), Templates::Conventional);
    }

    #[test]
    fn a_file_is_inside_a_templates_folder_in_any_case() {
        let conventional = Templates::Conventional;
        assert_eq!(conventional.holds(&path("Templates/a.md")), Some(1));
        assert_eq!(conventional.holds(&path("templates")), None);
        assert_eq!(conventional.holds(&path("notes/templates/a.md")), None);
        let nested = Templates::Folder(path("Meta/Tpl"));
        assert_eq!(nested.holds(&path("meta/tpl/deep/a.md")), Some(2));
        assert_eq!(nested.holds(&path("meta/a.md")), None);
        assert_eq!(nested.holds(&path("templates/a.md")), None);
    }
}
