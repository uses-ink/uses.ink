//! Writing files into an archive.

use super::{BLOCK, Error};

/// What a header says about its entry.
struct Head<'a> {
    name: &'a [u8],
    prefix: &'a [u8],
    flag: u8,
}

/// An archive being written in memory, up to a limit.
#[derive(Debug)]
pub struct Writer {
    out: Vec<u8>,
    limit: u64,
}

impl Writer {
    /// A writer that refuses to grow past `limit` bytes.
    pub const fn new(limit: u64) -> Self {
        Self { out: Vec::new(), limit }
    }

    /// Adds a file, the name written as given: no leading slash, parts joined by `/`.
    pub fn file(&mut self, name: &str, bytes: &[u8]) -> Result<(), Error> {
        let size = (self.out.len() as u64).saturating_add(bytes.len() as u64);
        if size > self.limit {
            return Err(Error::Total { size, limit: self.limit });
        }
        let (prefix, short) = Self::split(name);
        if short.len() > 100 || prefix.len() > 155 {
            let mut long = name.as_bytes().to_vec();
            long.push(0);
            self.entry(&Head { name: b"././@LongLink", prefix: b"", flag: b'L' }, &long);
        }
        let cut = |text: &'_ [u8], most: usize| text.get(..most.min(text.len())).unwrap_or_default().to_vec();
        self.entry(&Head { name: &cut(short, 100), prefix: &cut(prefix, 155), flag: b'0' }, bytes);
        Ok(())
    }

    /// The archive, closed with its two empty blocks.
    pub fn finish(mut self) -> Vec<u8> {
        self.out.resize(self.out.len() + 2 * BLOCK, 0);
        self.out
    }

    /// Splits a name at the last `/` that leaves a prefix a header holds, when it does.
    fn split(name: &str) -> (&[u8], &[u8]) {
        let bytes = name.as_bytes();
        let fits = |at: &usize| *at <= 155 && bytes.len() - at - 1 <= 100;
        match bytes.iter().enumerate().filter(|(_, byte)| **byte == b'/').map(|(at, _)| at).rev().find(fits) {
            Some(at) if bytes.len() > 100 => {
                (bytes.get(..at).unwrap_or_default(), bytes.get(at + 1..).unwrap_or_default())
            }
            _ => (b"", bytes),
        }
    }

    /// One header and its data, padded to a block.
    fn entry(&mut self, head: &Head<'_>, data: &[u8]) {
        let Head { name, prefix, flag } = *head;
        let mut block = [0_u8; BLOCK];
        let mut put = |from: usize, bytes: &[u8]| {
            for (slot, byte) in block.iter_mut().skip(from).zip(bytes) {
                *slot = *byte;
            }
        };
        put(0, name);
        put(100, b"0000644\0");
        put(108, b"0000000\0");
        put(116, b"0000000\0");
        put(124, format!("{:011o}\0", data.len()).as_bytes());
        put(136, b"00000000000\0");
        put(148, b"        ");
        put(156, &[flag]);
        put(257, b"ustar\x0000");
        put(345, prefix);
        let sum: u32 = block.iter().map(|byte| u32::from(*byte)).sum();
        for (slot, byte) in block.iter_mut().skip(148).zip(format!("{sum:06o}\0 ").bytes()) {
            *slot = byte;
        }
        self.out.extend_from_slice(&block);
        self.out.extend_from_slice(data);
        self.out.resize(self.out.len().next_multiple_of(BLOCK), 0);
    }
}

#[cfg(test)]
mod test {
    use model::ContentPath;

    use super::*;
    use crate::archive::{Limits, Tree};

    const LIMITS: Limits = Limits { files: 8, file: 1024, total: 4096, name: 512 };

    fn path(text: &str) -> ContentPath {
        ContentPath::parse(text).unwrap()
    }

    #[test]
    fn what_is_written_is_read_back_whatever_the_name() {
        let long = format!("{}/{}.html", "a".repeat(90), "b".repeat(90));
        let longer = "c".repeat(300);
        let mut writer = Writer::new(4096);
        for (name, bytes) in [("index.html", "hello"), ("posts/a.html", ""), (&long, "split"), (&longer, "gnu")] {
            writer.file(name, bytes.as_bytes()).unwrap();
        }
        let tree = Tree::read(&writer.finish(), &LIMITS).unwrap();
        assert_eq!(tree.len(), 4);
        assert_eq!(tree.get(&path("index.html")), Some(b"hello".as_slice()));
        assert_eq!(tree.get(&path("posts/a.html")), Some(b"".as_slice()));
        assert_eq!(tree.get(&path(&long)), Some(b"split".as_slice()));
        assert_eq!(tree.get(&path(&longer)), Some(b"gnu".as_slice()));
    }

    #[test]
    fn the_writer_stops_at_its_limit() {
        let mut writer = Writer::new(1000);
        writer.file("a", &[0; 100]).unwrap();
        let error = writer.file("b", &[0; 900]).unwrap_err();
        assert!(matches!(error, Error::Total { limit: 1000, .. }));
    }

    #[test]
    fn dot_slash_names_are_the_folder_they_are_in() {
        let mut writer = Writer::new(4096);
        writer.file("./index.html", b"x").unwrap();
        assert!(Tree::read(&writer.finish(), &LIMITS).unwrap().get(&path("index.html")).is_some());
    }
}
