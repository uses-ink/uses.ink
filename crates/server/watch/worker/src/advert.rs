//! The reference advertisement.

use std::collections::HashMap;

use build::Branch;
use watch::{Branches, Head, HeadError, Skipped, Tip};

use crate::{Absent, Misread};

/// What a smart server lists for `service=git-upload-pack`: the references and the branch `HEAD`
/// points at. Packets are `<4 hex digits of length><payload>` and `0000` is a flush; the first
/// reference line carries the capabilities after a NUL byte, among them `symref=HEAD:<branch>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Advertisement {
    direct: Vec<(String, String)>,
    peeled: HashMap<String, String>,
    symref: Option<String>,
    unborn: bool,
}

impl Advertisement {
    /// The preamble every smart server sends first.
    const SERVICE: &'static [u8] = b"# service=git-upload-pack";

    /// The id git writes for a reference that has none (an empty repository).
    const NONE: &'static str = "0000000000000000000000000000000000000000";

    /// Reads the body of the `info/refs` answer.
    pub fn parse(body: &[u8]) -> Result<Self, Misread> {
        let mut at = 0;
        let first = Self::packet(body, &mut at)?;
        let line = first.map(|payload| payload.strip_suffix(b"\n").unwrap_or(payload));
        if line != Some(Self::SERVICE) || Self::packet(body, &mut at)?.is_some() {
            return Err(Misread::Service);
        }
        let mut found = Self::default();
        let mut opening = true;
        loop {
            let start = at;
            let Some(payload) = Self::packet(body, &mut at)? else { break };
            if payload == b"version 2\n" || payload == b"version 2" {
                return Err(Misread::Version);
            }
            let (line, capabilities) = match payload.iter().position(|byte| *byte == 0) {
                Some(split) => (payload.get(..split).unwrap_or_default(), payload.get(split + 1..)),
                None => (payload, None),
            };
            if opening {
                found.symref = capabilities.and_then(Self::symref);
                opening = false;
            }
            found.line(line.strip_suffix(b"\n").unwrap_or(line), start)?;
        }
        Ok(found)
    }

    /// The packet at `at`: its payload, or none for a flush. `at` moves past it.
    fn packet<'a>(body: &'a [u8], at: &mut usize) -> Result<Option<&'a [u8]>, Misread> {
        let start = *at;
        let length = body.get(start..start + 4).ok_or(Misread::Truncated { at: start })?;
        let text = std::str::from_utf8(length).map_err(|_| Misread::Length { at: start })?;
        let length = usize::from_str_radix(text, 16).map_err(|_| Misread::Length { at: start })?;
        *at = start + length.max(4);
        match length {
            0 => Ok(None),
            1..=3 => Err(Misread::Length { at: start }),
            _ => body.get(start + 4..start + length).map(Some).ok_or(Misread::Truncated { at: start }),
        }
    }

    /// The branch `symref=HEAD:<target>` names, among the capabilities.
    fn symref(capabilities: &[u8]) -> Option<String> {
        let text = String::from_utf8_lossy(capabilities);
        let target = text.split(' ').find_map(|word| word.strip_prefix("symref=HEAD:"))?;
        Some(target.trim_end().to_owned())
    }

    /// Takes one `<id> <name>` line.
    fn line(&mut self, line: &[u8], at: usize) -> Result<(), Misread> {
        let text = std::str::from_utf8(line).map_err(|_| Misread::Line { at })?;
        let (id, name) = text.split_once(' ').ok_or(Misread::Line { at })?;
        if id == Self::NONE && name == "capabilities^{}" {
            self.unborn = true;
        } else if let Some(tag) = name.strip_suffix("^{}") {
            self.peeled.insert(tag.to_owned(), id.to_owned());
        } else {
            self.direct.push((name.to_owned(), id.to_owned()));
        }
        Ok(())
    }

    /// The commit a reference of this full name points at, tags peeled.
    fn named(&self, name: &str) -> Option<&str> {
        self.peeled
            .get(name)
            .map(String::as_str)
            .or_else(|| self.direct.iter().find(|(found, _)| found == name).map(|(_, id)| id.as_str()))
    }

    /// The branch `HEAD` points at, by its short name.
    pub fn default_branch(&self) -> Option<&str> {
        self.symref.as_deref().and_then(|target| target.strip_prefix("refs/heads/"))
    }

    /// The commit `revision` names, or the default branch's when none is named: a branch, then a
    /// tag, then a full reference name, then a commit id written out, which names itself.
    pub fn head(&self, revision: Option<&str>) -> Result<Head, HeadOr> {
        let Some(written) = revision else {
            let id =
                self.named("HEAD").ok_or(HeadOr::Absent(if self.unborn { Absent::Unborn } else { Absent::NoHead }))?;
            return id.parse().map_err(HeadOr::Commit);
        };
        let found = [format!("refs/heads/{written}"), format!("refs/tags/{written}"), written.to_owned()]
            .iter()
            .find_map(|name| self.named(name));
        match found {
            Some(id) => id.parse().map_err(HeadOr::Commit),
            None => written
                .to_ascii_lowercase()
                .parse::<Head>()
                .map_err(|_| HeadOr::Absent(Absent::Revision { written: written.to_owned() })),
        }
    }

    /// Every branch the remote advertises, with its commit, in the order of their names, the
    /// default among them marked, and the references that are no branch to this set aside.
    pub fn branches(&self) -> Result<Branches, HeadError> {
        let mut named: Vec<(&str, &str)> = self
            .direct
            .iter()
            .filter_map(|(name, id)| name.strip_prefix("refs/heads/").map(|branch| (branch, id.as_str())))
            .collect();
        named.sort_by(|left, right| left.0.cmp(right.0));
        let mut read = Branches::default();
        for (written, id) in named {
            let head: Head = id.parse()?;
            match written.parse::<Branch>() {
                Ok(branch) => read.found.push(Tip { branch, head }),
                Err(source) => read.skipped.push(Skipped { written: written.to_owned(), source }),
            }
        }
        read.default = self.default_branch().and_then(|name| name.parse().ok());
        Ok(read)
    }
}

/// Why `head` found no commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadOr {
    /// The remote has none.
    Absent(Absent),
    /// The remote named an id that is no commit id.
    Commit(HeadError),
}
