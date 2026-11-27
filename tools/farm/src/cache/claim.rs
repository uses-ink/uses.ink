//! The jobs running now, so an identical job attaches to the one running.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::Fault;
use crate::cache::hash::Hash;
use crate::cache::hash::key::Key;
use crate::cache::store::Files;

/// A job running now, which an identical job may attach to, and what a pass of it will be keyed by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Mark {
    pub(crate) id: String,
    /// The process that started it, which is still working for it while it is alive.
    pub(crate) pid: u32,
    pub(crate) key: Hash,
    pub(crate) job: String,
    pub(crate) command: String,
    pub(crate) workspace: String,
    pub(crate) host: String,
    pub(crate) digest: Hash,
}

/// How a claim on a running job ended.
#[derive(Debug)]
pub(crate) enum Claim {
    /// No identical job was running: this one is, until the guard drops.
    Won(Claimed),
    /// One is: attach to it.
    Held(Mark),
}

/// A claim on a running job, released when it drops unless it was told to stay: a detached job
/// is still running when its client leaves.
#[derive(Debug)]
pub(crate) struct Claimed {
    marks: Marks,
    key: Key,
    stay: bool,
}

impl Claimed {
    /// Leaves the mark in place: the job outlives the client.
    pub(crate) const fn stay(&mut self) {
        self.stay = true;
    }
}

impl Drop for Claimed {
    fn drop(&mut self) {
        if !self.stay {
            self.marks.release(self.key);
        }
    }
}

/// The jobs running now.
#[derive(Debug, Clone)]
pub(crate) struct Marks {
    dir: PathBuf,
}

impl Marks {
    pub(crate) fn new(dir: &Path) -> Self {
        Self { dir: dir.join("running") }
    }

    fn path(&self, key: Key) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// Marks the job as running, or says which identical job already is. The mark is made whole
    /// by a hard link, which fails if one exists, so two clients never both win.
    pub(crate) fn claim(&self, key: Key, mark: &Mark) -> Result<Claim, Fault> {
        let path = self.path(key);
        let io = |source| Fault::Io { path: path.clone(), source };
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let temp = self.dir.join(format!("{}.tmp", mark.id));
        let text = serde_json::to_string(mark).map_err(|source| Fault::Json { path: path.clone(), source })?;
        std::fs::write(&temp, text).map_err(io)?;
        for _ in 0..3 {
            match std::fs::hard_link(&temp, &path) {
                Ok(()) => {
                    std::fs::remove_file(&temp).map_err(io)?;
                    return Ok(Claim::Won(Claimed { marks: self.clone(), key, stay: false }));
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => match Files::read::<Mark>(&path) {
                    Ok(Some(held)) => {
                        std::fs::remove_file(&temp).map_err(io)?;
                        return Ok(Claim::Held(held));
                    }
                    // released since the link failed, or unreadable: take it away and try again
                    Ok(None) => {}
                    Err(Fault::Json { .. }) => self.release(key),
                    Err(other) => return Err(other),
                },
                Err(source) => return Err(io(source)),
            }
        }
        std::fs::remove_file(&temp).ok();
        Err(io(std::io::Error::other("the mark kept changing under the claim")))
    }

    /// Takes the mark away; a mark that is gone is fine.
    pub(crate) fn release(&self, key: Key) {
        std::fs::remove_file(self.path(key)).ok();
    }

    /// The running job with this id, and the key of its mark.
    pub(crate) fn holding(&self, id: &str) -> Result<Option<(Key, Mark)>, Fault> {
        let listing = match std::fs::read_dir(&self.dir) {
            Ok(listing) => listing,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Fault::Io { path: self.dir.clone(), source }),
        };
        for item in listing {
            let path = item.map_err(|source| Fault::Io { path: self.dir.clone(), source })?.path();
            let stem = path.file_stem().and_then(|stem| stem.to_str()).and_then(|stem| stem.parse::<Hash>().ok());
            let (true, Some(stem)) = (path.extension().is_some_and(|extension| extension == "json"), stem) else {
                continue;
            };
            if let Some(mark) = Files::read::<Mark>(&path)?.filter(|mark| mark.id == id) {
                return Ok(Some((Key(stem), mark)));
            }
        }
        Ok(None)
    }

    /// Whether the process that started a job still runs.
    pub(crate) fn alive(pid: u32) -> bool {
        Path::new("/proc").join(pid.to_string()).exists()
    }

    /// The key a mark was filed under, for a mark read back.
    pub(crate) const fn key(hash: Hash) -> Key {
        Key(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::hash::Hasher;
    use crate::cache::tests::{Dir, facts};

    #[test]
    fn two_identical_jobs_do_not_both_claim_and_a_released_claim_can_be_taken() {
        let dir = Dir::new("marks");
        let marks = Marks::new(&dir.0);
        let words = vec!["x".to_owned()];
        let key = Key::of(&facts(&words, 1)).within("w");
        let mark = |id: &str| Mark {
            id: id.to_owned(),
            pid: std::process::id(),
            key: Hasher::default().finish(),
            job: "test".to_owned(),
            command: "c".to_owned(),
            workspace: "w".to_owned(),
            host: "petal".to_owned(),
            digest: Hasher::default().finish(),
        };
        let Claim::Won(mut first) = marks.claim(key, &mark("petal.1")).unwrap() else { panic!("the first claim wins") };
        let Claim::Held(held) = marks.claim(key, &mark("petal.2")).unwrap() else {
            panic!("the second finds the first")
        };
        assert_eq!(held.id, "petal.1");
        assert_eq!(marks.holding("petal.1").unwrap().map(|(found, _)| found), Some(key));
        first.stay();
        drop(first);
        assert!(matches!(marks.claim(key, &mark("petal.3")).unwrap(), Claim::Held(_)), "a detached job keeps its mark");
        marks.release(key);
        let Claim::Won(claimed) = marks.claim(key, &mark("petal.4")).unwrap() else { panic!("free again") };
        drop(claimed);
        assert!(marks.holding("petal.4").unwrap().is_none(), "a claim that drops releases");
    }
}
