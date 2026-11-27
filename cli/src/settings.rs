//! What a run is set by besides its flags: the environment and `~/.config/ink/config.toml`.
//!
//! One precedence for every setting: the flag, then the environment, then the user's config file,
//! then the default (`docs/cli.md` section 13). Clap reads the flag and its variable; this reads
//! the file and says which wins. The site's own `ink.toml` never holds a terminal preference: a
//! site is shared, a terminal is not.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::grammar::args::{Cli, When};

/// Where variables come from, so a test needs no process environment.
pub(crate) trait Environment {
    /// The variable `name`, when it is set to text.
    fn get(&self, name: &str) -> Option<String>;
}

/// The process's own environment.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Process;

impl Environment for Process {
    fn get(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// How progress is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Progress {
    /// A live line on a terminal, log lines elsewhere.
    #[default]
    Live,
    /// Log lines only.
    Plain,
}

/// Which glyphs are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Glyphs {
    /// Box drawing and marks.
    Unicode,
    /// Plain ASCII.
    Ascii,
}

/// How times in listings are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Times {
    /// UTC, ISO.
    Utc,
    /// The machine's own zone.
    Local,
}

/// The `[ui]` table of the config file: the person's terminal preferences.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Ui {
    pub(crate) color: Option<When>,
    pub(crate) glyphs: Option<Glyphs>,
    pub(crate) progress: Option<Progress>,
    pub(crate) times: Option<Times>,
    pub(crate) width: Option<u16>,
}

/// The `[log]` table: what the log says when no flag or variable does (`docs/tracing.md`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Log {
    /// An `INK_LOG` filter.
    pub(crate) filter: Option<String>,
    /// A file for JSON lines.
    pub(crate) file: Option<PathBuf>,
    /// `pretty`, `compact` or `json`.
    pub(crate) format: Option<String>,
}

/// The `[account]` table: which API and which site the remote commands use when the flags and the
/// environment say nothing. There is no token: a machine's private key is kept by `ink login` in its
/// own file, never in this one, and an old `token` key is an error that names it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Account {
    pub(crate) api: Option<String>,
    pub(crate) site: Option<String>,
}

/// The user's config file, parsed.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Preferences {
    pub(crate) ui: Ui,
    pub(crate) log: Log,
    pub(crate) account: Account,
}

/// The config file could not be read.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unreadable {
    /// The file exists and cannot be read.
    Read {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The file is not the TOML this reads.
    Parse {
        /// The file.
        path: PathBuf,
        /// Why not, with the line.
        source: toml::de::Error,
    },
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(out, "the config file {} could not be read", path.display()),
            Self::Parse { path, .. } => write!(out, "the config file {} is not valid", path.display()),
        }
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

impl Preferences {
    /// Where the file is: `$XDG_CONFIG_HOME/ink/config.toml`, else `~/.config/ink/config.toml`.
    pub(crate) fn path(env: &impl Environment) -> Option<PathBuf> {
        let base = env
            .get("XDG_CONFIG_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| env.get("HOME").map(|home| Path::new(&home).join(".config")))?;
        Some(base.join("ink").join("config.toml"))
    }

    /// The file's preferences; none when there is no file.
    pub(crate) fn load(env: &impl Environment) -> Result<Self, Unreadable> {
        let Some(path) = Self::path(env) else { return Ok(Self::default()) };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|source| Unreadable::Parse { path, source }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Unreadable::Read { path, source }),
        }
    }
}

/// The terminal settings of one run, each decided by the one precedence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// When to colour.
    pub color: When,
    /// How progress is shown.
    pub progress: Progress,
}

impl Settings {
    /// The settings `cli` asks for, with `preferences` under them and the defaults under those.
    /// The flag and its variable were read by clap.
    pub(crate) fn of(cli: &Cli, preferences: &Preferences) -> Self {
        let progress = if cli.no_progress { Some(Progress::Plain) } else { preferences.ui.progress };
        Self { color: cli.color.or(preferences.ui.color).unwrap_or_default(), progress: progress.unwrap_or_default() }
    }
}

#[cfg(test)]
mod test {
    use clap::Parser;

    use super::*;

    #[test]
    fn the_flag_beats_the_file_and_the_file_beats_the_default() {
        let preferences: Preferences =
            toml::from_str("[ui]\ncolor = \"never\"\nprogress = \"plain\"\n").unwrap_or_default();
        let flagged = Cli::try_parse_from(["ink", "--color", "always", "man"]).unwrap();
        let bare = Cli::try_parse_from(["ink", "man"]).unwrap();
        assert_eq!(Settings::of(&flagged, &preferences).color, When::Always);
        assert_eq!(Settings::of(&bare, &preferences).color, When::Never);
        assert_eq!(Settings::of(&bare, &preferences).progress, Progress::Plain);
        assert_eq!(Settings::of(&bare, &Preferences::default()).color, When::Auto);
    }

    #[test]
    fn an_unknown_key_is_an_error_and_a_token_is_not_a_setting_any_more() {
        assert!(toml::from_str::<Preferences>("[ui]\ncolour = \"never\"\n").is_err());
        let error = toml::from_str::<Preferences>("[account]\ntoken = \"secret\"\n").unwrap_err();
        assert!(error.to_string().contains("token"), "{error}");
    }
}
