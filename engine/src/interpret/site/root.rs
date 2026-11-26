//! `site.root`: where the site's content starts in the snapshot.

use model::{ContentPath, DefaultReason, Evidence, Root, Snapshot, Subject};

use crate::Format;
use crate::interpret::file::ignore;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Folders that hold a site's content when they are the only place content is. Not `posts` or
/// `blog`: a root there would make every post a plain page, as `page.kind` reads a posts folder
/// below the root.
const FOLDERS: &[&str] = &["content", "docs", "notes", "src/content"];

/// Pin, then the one content folder of [`FOLDERS`] when every file a format reads is in it
/// (repository files such as `README.md` and `LICENSE` at the top aside), then the top.
pub(crate) fn decide(snapshot: &Snapshot, formats: &[&dyn Format], pins: &impl Pins, record: &mut Record) -> Root {
    let found =
        folder(snapshot, formats).map(|folder| (Root::Folder(folder.clone()), Evidence::Marker { path: folder }));
    Ladder::<code::Root>::new(Subject::Site, pins, record).rung(found).or_default(
        Root::Top,
        DefaultReason::NothingMatched,
        record,
    )
}

fn folder(snapshot: &Snapshot, formats: &[&dyn Format]) -> Option<ContentPath> {
    let present: Vec<ContentPath> = FOLDERS
        .iter()
        .filter_map(|folder| ContentPath::parse(*folder).ok())
        .filter(|folder| snapshot.tree().iter().any(|entry| entry.path.strip(folder).is_some()))
        .collect();
    let [only] = present.as_slice() else { return None };
    let stray = snapshot.tree().iter().any(|entry| {
        entry.path.strip(only).is_none()
            && formats.iter().any(|format| format.handles(&entry.path))
            && !ignore::is_repository_file(&entry.path)
    });
    (!stray).then(|| only.clone())
}
