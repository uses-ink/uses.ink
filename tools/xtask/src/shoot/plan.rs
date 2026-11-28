//! The plan the task hands the browser's driver, `tools/shoot/shoot.mjs`, as JSON.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::fault::Fault;

/// One page to take a picture of, at every width.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Shot {
    /// The picture's name: `<name>-<width>.png`.
    pub(crate) name: String,
    /// Where the browser loads it.
    pub(crate) url: String,
}

/// A folder the driver serves itself, for what `ink serve` does not serve: the mock.
#[derive(Debug, Serialize)]
pub(crate) struct Static {
    /// The folder.
    pub(crate) root: PathBuf,
    /// The loopback port.
    pub(crate) port: u16,
}

/// What the driver is asked: the pictures, or only whether a browser starts.
#[derive(Debug, Serialize)]
pub(crate) struct Plan {
    /// True to start the browser, draw one page and say nothing else.
    pub(crate) probe: bool,
    /// Where the pictures are written.
    pub(crate) out: PathBuf,
    /// The viewport widths.
    pub(crate) widths: Vec<u32>,
    /// The pages.
    pub(crate) shots: Vec<Shot>,
    /// A folder to serve for the shots, if any.
    pub(crate) serve: Option<Static>,
}

impl Plan {
    /// Writes it to `path`, and the folder above it.
    pub(crate) fn write(&self, path: &Path) -> Result<(), Fault> {
        let file = |source| Fault::File { path: path.to_path_buf(), source };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(file)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(Fault::Plan)?;
        std::fs::write(path, json).map_err(file)
    }
}
