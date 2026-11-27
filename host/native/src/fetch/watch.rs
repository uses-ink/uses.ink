use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use model::Bytes;
use model::git::{Budget, Refused};

use super::Failed;

/// Watches a clone's folder while gix fills it, and interrupts the fetch past the byte limit or
/// the deadline.
pub(super) struct Watch<'f> {
    pub(super) folder: &'f Path,
    pub(super) limit: Bytes,
    pub(super) deadline: Option<Instant>,
}

impl Watch<'_> {
    /// How often the folder is measured.
    const EVERY: Duration = Duration::from_millis(10);

    /// Runs `fetch` with a watcher beside it; a fetch the watcher cut, or that ended past the
    /// limit, removes the clone and is the budget's refusal.
    pub(super) fn over<T>(&self, fetch: impl FnOnce(&AtomicBool) -> Result<T, gix::Error>) -> Result<T, Refused<Failed>> {
        let (interrupt, done, late) = (AtomicBool::new(false), AtomicBool::new(false), AtomicBool::new(false));
        let result = std::thread::scope(|scope| {
            scope.spawn(|| {
                while !done.load(Ordering::Relaxed) {
                    if self.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        late.store(true, Ordering::Relaxed);
                        interrupt.store(true, Ordering::Relaxed);
                    }
                    if Footprint(self.folder).bytes().is_ok_and(|size| size > self.limit) {
                        interrupt.store(true, Ordering::Relaxed);
                    }
                    std::thread::sleep(Self::EVERY);
                }
            });
            let result = fetch(&interrupt);
            done.store(true, Ordering::Relaxed);
            result
        });
        if late.load(Ordering::Relaxed) {
            self.remove()?;
            return Err(Refused::Failed(Failed::Late));
        }
        // an unmeasured clone is not taken as empty: the budget cannot pass what it cannot count
        let size = Footprint(self.folder)
            .bytes()
            .map_err(|source| Refused::Failed(Failed::Cache { path: self.folder.to_owned(), source }))?;
        if interrupt.load(Ordering::Relaxed) || (result.is_ok() && size > self.limit) {
            self.remove()?;
            return Err(Refused::Budget(Budget::Bytes { size, limit: self.limit }));
        }
        result.map_err(|source| Refused::Failed(Failed::Fetch { source }))
    }

    /// Removes the clone: the host made the folder, and a clone past a budget is not kept.
    pub(super) fn remove(&self) -> Result<(), Refused<Failed>> {
        match std::fs::remove_dir_all(self.folder) {
            Err(source) if source.kind() != std::io::ErrorKind::NotFound => {
                Err(Refused::Failed(Failed::Cache { path: self.folder.to_owned(), source }))
            }
            _ => Ok(()),
        }
    }
}

/// How much disk a folder's files take.
pub(super) struct Footprint<'f>(pub(super) &'f Path);

impl Footprint<'_> {
    pub(super) fn bytes(&self) -> std::io::Result<Bytes> {
        let mut total = 0_u64;
        let mut pending = vec![self.0.to_owned()];
        while let Some(folder) = pending.pop() {
            let entries = match std::fs::read_dir(&folder) {
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
                other => other?,
            };
            for entry in entries {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                match entry.metadata() {
                    Ok(metadata) => total = total.saturating_add(metadata.len()),
                    // gone since it was listed, so it takes no disk; any other failure is the
                    // budget's to know, since an unmeasured file could be any size
                    Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                    Err(source) => return Err(source),
                }
            }
        }
        Ok(Bytes(total))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_fetch_that_stalls_is_cut_at_the_deadline() {
        let folder = Path::new("a-clone-that-is-never-made");
        let watch = Watch { folder, limit: Bytes(1 << 30), deadline: Some(Instant::now() + Duration::from_millis(50)) };
        let started = Instant::now();
        let stalled = watch.over::<()>(|interrupt| {
            while !interrupt.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(gix::url::parse(gix::bstr::BStr::new("")).unwrap_err())
        });
        assert!(matches!(stalled, Err(Refused::Failed(Failed::Late))), "{stalled:?}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
