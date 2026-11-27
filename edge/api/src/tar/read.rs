//! Reading an archive into a tree of files.

use std::collections::BTreeMap;

use model::ContentPath;

use super::{BLOCK, Error};

/// What an archive may hold: every number is a ceiling the reader checks as it goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Files in the archive.
    pub files: u64,
    /// Bytes in one file.
    pub file: u64,
    /// Bytes in every file together.
    pub total: u64,
    /// Bytes in a file's name.
    pub name: usize,
}

/// The files of an archive by path, with their bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree(BTreeMap<ContentPath, Vec<u8>>);

impl Tree {
    /// The bytes of the file at `path`.
    pub fn get(&self, path: &ContentPath) -> Option<&[u8]> {
        self.0.get(path).map(Vec::as_slice)
    }

    /// How many files.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Reads `bytes` as an archive of files within `limits`.
    pub fn read(bytes: &[u8], limits: &Limits) -> Result<Self, Error> {
        let mut reading = Reading { bytes, at: 0, limits, long: None, total: 0, tree: Self::default() };
        while reading.next()? {}
        Ok(reading.tree)
    }
}

/// An archive being read: where it is, the long name the last entry gave, what has been taken.
struct Reading<'a> {
    bytes: &'a [u8],
    at: usize,
    limits: &'a Limits,
    long: Option<Vec<u8>>,
    total: u64,
    tree: Tree,
}

impl<'a> Reading<'a> {
    /// Takes the next entry. False at the end of the archive.
    fn next(&mut self) -> Result<bool, Error> {
        let at = self.at;
        if at == self.bytes.len() {
            return Ok(false);
        }
        let block = self.bytes.get(at..at + BLOCK).ok_or(Error::Truncated { at: self.bytes.len() })?;
        if block.iter().all(|byte| *byte == 0) {
            return Ok(false);
        }
        let header = Header { at, block };
        header.verify()?;
        let size = header.size()?;
        let start = at + BLOCK;
        let data = self.data(start, size, header.flag())?;
        self.at = start + usize::try_from(size.next_multiple_of(BLOCK as u64)).unwrap_or(usize::MAX);
        let taken = match header.flag() {
            b'0' | 0 => self.file(&header, data),
            b'5' => {
                self.long = None;
                Ok(())
            }
            b'L' => {
                self.name(data);
                Ok(())
            }
            flag => Err(Error::Kind { at, flag }),
        };
        taken.map(|()| true)
    }

    /// The `size` bytes of an entry's data, checked against the limit for its kind first.
    fn data(&self, start: usize, size: u64, flag: u8) -> Result<&'a [u8], Error> {
        let bytes = self.bytes;
        match flag {
            b'L' if size > self.limits.name as u64 => {
                return Err(Error::Long { size: usize::try_from(size).unwrap_or(usize::MAX), limit: self.limits.name });
            }
            b'0' | 0 if size > self.limits.file => {
                let name = String::from_utf8_lossy(self.name_at(start - BLOCK)).into_owned();
                return Err(Error::File { name, size, limit: self.limits.file });
            }
            _ => {}
        }
        let end = usize::try_from(size).ok().and_then(|size| start.checked_add(size));
        end.and_then(|end| bytes.get(start..end)).ok_or(Error::Truncated { at: bytes.len() })
    }

    /// The raw name field of the header at `at`, up to its NUL.
    fn name_at(&self, at: usize) -> &[u8] {
        let field = self.bytes.get(at..at + 100).unwrap_or_default();
        field.split(|byte| *byte == 0).next().unwrap_or_default()
    }

    /// Remembers a GNU long name for the entry after it.
    fn name(&mut self, data: &[u8]) {
        let end = data.iter().position(|byte| *byte == 0).unwrap_or(data.len());
        self.long = Some(data.get(..end).unwrap_or_default().to_vec());
    }

    /// Takes a regular file.
    fn file(&mut self, header: &Header<'_>, data: &[u8]) -> Result<(), Error> {
        let path = header.path(self.long.take())?;
        let limits = self.limits;
        if self.tree.len() as u64 >= limits.files {
            return Err(Error::Files { limit: limits.files });
        }
        self.total = self.total.saturating_add(data.len() as u64);
        if self.total > limits.total {
            return Err(Error::Total { size: self.total, limit: limits.total });
        }
        match self.tree.0.insert(path.clone(), data.to_vec()) {
            Some(_) => Err(Error::Duplicate { name: path.as_str().to_owned() }),
            None => Ok(()),
        }
    }
}

/// One header block.
struct Header<'a> {
    at: usize,
    block: &'a [u8],
}

impl Header<'_> {
    /// `range` of the block.
    fn field(&self, from: usize, to: usize) -> &[u8] {
        self.block.get(from..to).unwrap_or_default()
    }

    fn flag(&self) -> u8 {
        self.block.get(156).copied().unwrap_or_default()
    }

    /// An octal number field.
    fn octal(&self, from: usize, to: usize) -> Result<u64, Error> {
        let text: Vec<u8> = self.field(from, to).iter().copied().filter(|byte| !matches!(byte, 0 | b' ')).collect();
        text.iter().try_fold(0_u64, |value, digit| match digit {
            b'0'..=b'7' => {
                value.checked_mul(8).map(|value| value | u64::from(digit - b'0')).ok_or(Error::Number { at: self.at })
            }
            _ => Err(Error::Number { at: self.at }),
        })
    }

    fn size(&self) -> Result<u64, Error> {
        self.octal(124, 136)
    }

    /// Checks the sum of the header's bytes, the checksum field counted as spaces.
    fn verify(&self) -> Result<(), Error> {
        let sum: u64 = self
            .block
            .iter()
            .enumerate()
            .map(|(index, byte)| if (148..156).contains(&index) { 32 } else { u64::from(*byte) })
            .sum();
        if sum == self.octal(148, 156)? { Ok(()) } else { Err(Error::Checksum { at: self.at }) }
    }

    /// The entry's path: the long name an earlier entry gave, or the ustar prefix and name.
    fn path(&self, long: Option<Vec<u8>>) -> Result<ContentPath, Error> {
        let raw = if let Some(long) = long {
            long
        } else {
            let cut = |bytes: &[u8]| bytes.split(|byte| *byte == 0).next().unwrap_or_default().to_vec();
            let name = cut(self.field(0, 100));
            let prefix = cut(self.field(345, 500));
            let ustar = self.field(257, 262) == b"ustar";
            if ustar && !prefix.is_empty() { [prefix, b"/".to_vec(), name].concat() } else { name }
        };
        let text = String::from_utf8(raw).map_err(|_| Error::Name { at: self.at, source: None })?;
        let text = text.strip_prefix("./").unwrap_or(&text);
        ContentPath::parse(text).map_err(|source| Error::Name { at: self.at, source: Some(source) })
    }
}
