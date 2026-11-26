//! `file.ignore`: files under the root that are never part of the site.

use model::{ContentPath, DefaultReason, Evidence};

use crate::interpret::file::scope::{InRoot, Scope};
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Stems of files about the repository, not the site, when they sit at the top of the root.
const REPOSITORY: &[&str] = &["license", "licence", "changelog", "contributing", "code_of_conduct", "security"];

/// Folders of other tools' dependencies.
const VENDORED: &[&str] = &["node_modules"];

/// The config file, at the top of the snapshot.
const CONFIG: &str = "ink.toml";

/// Pin, then an `exclude` pattern, then an `include` pattern, then the rules (dotfiles and dot
/// folders, vendored folders, repository files, `ink.toml`), then kept. Returns the files kept.
pub(crate) fn decide<'a>(scope: &Scope<'a>, pins: &impl Pins, record: &mut Record) -> Vec<InRoot<'a>> {
    let mut kept = Vec::new();
    for file in &scope.inside {
        let ruled = is_ruled_out(file).then(|| (true, Evidence::Filename { path: file.path().clone() }));
        let ignored = Ladder::<code::Ignored>::new(file.subject(), pins, record)
            .rung(pins.excluded(file.path()).map(|span| (true, Evidence::Config { span })))
            .rung(pins.included(file.path()).map(|span| (false, Evidence::Config { span })))
            .rung(ruled)
            .or_default(false, DefaultReason::NothingMatched, record);
        if !ignored {
            kept.push(file.clone());
        }
    }
    kept
}

fn is_ruled_out(file: &InRoot<'_>) -> bool {
    let relative = &file.relative;
    relative.segments().any(|segment| segment.starts_with('.') || VENDORED.contains(&segment))
        || (relative.parent().is_none() && is_repository_name(relative))
        || file.path().as_str() == CONFIG
}

fn is_repository_name(path: &ContentPath) -> bool {
    let stem = path.stem().to_ascii_lowercase();
    REPOSITORY.iter().any(|name| stem.starts_with(name))
}

/// Whether a snapshot path is about the repository rather than any site in it: a dotfile, or a
/// README or repository file at the top. `site.root` looks past these.
pub(crate) fn is_repository_file(path: &ContentPath) -> bool {
    let top = path.parent().is_none();
    path.segments().any(|segment| segment.starts_with('.'))
        || (top && (is_repository_name(path) || path.stem().eq_ignore_ascii_case("readme")))
}
