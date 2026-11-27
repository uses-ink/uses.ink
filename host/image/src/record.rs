//! One image's work as the values a cache keeps: a record of what was made (its size, each
//! copy's format and size, whether there is a placeholder), then each copy's bytes in order,
//! then the placeholder's. The record is little-endian binary behind [`Record::START`]; the
//! bytes are as they were made, so what comes back from a cache is what was put in it.

use engine::image::{Encoded, Kept, Prepared, Variants};
use model::{Dimensions, MediaType};

/// The values of one image's work.
#[derive(Debug)]
pub(crate) struct Record;

/// A record being read.
#[derive(Debug)]
struct Reader<'a>(&'a [u8]);

impl Record {
    /// How every record starts; a new layout is a new version here and in `Key::VERSION`.
    const START: &'static [u8] = b"uses.ink/image/record/v1\n";

    /// `prepared` as the values a cache keeps, or `None` when it holds something a record
    /// cannot say (a kind of keeping or a format added since), which is then not kept.
    pub(crate) fn values(prepared: &Prepared) -> Option<Vec<Vec<u8>>> {
        let mut record = Self::START.to_vec();
        let mut values = Vec::new();
        match prepared {
            Prepared::Kept { dimensions, reason } => {
                record.push(0);
                Self::size(&mut record, *dimensions);
                record.push(match reason {
                    Kept::Animated => 0,
                    Kept::Format => 1,
                    _ => return None,
                });
            }
            Prepared::Varied(variants) => {
                record.push(1);
                Self::size(&mut record, variants.dimensions);
                record.extend_from_slice(&u32::try_from(variants.copies.len()).ok()?.to_le_bytes());
                for copy in &variants.copies {
                    Self::copy(&mut record, &mut values, copy)?;
                }
                record.push(u8::from(variants.placeholder.is_some()));
                if let Some(placeholder) = &variants.placeholder {
                    Self::copy(&mut record, &mut values, placeholder)?;
                }
            }
            _ => return None,
        }
        values.insert(0, record);
        Some(values)
    }

    /// The work the values of [`Record::values`] say, or `None` when they do not read as one.
    pub(crate) fn read(values: Vec<Vec<u8>>) -> Option<Prepared> {
        let mut values = values.into_iter();
        let record = values.next()?;
        let mut reader = Reader(record.strip_prefix(Self::START)?);
        let prepared = match reader.byte()? {
            0 => {
                let dimensions = reader.size()?;
                let reason = match reader.byte()? {
                    0 => Kept::Animated,
                    1 => Kept::Format,
                    _ => return None,
                };
                Prepared::Kept { dimensions, reason }
            }
            1 => {
                let dimensions = reader.size()?;
                let count = reader.word()?;
                let mut copies = Vec::new();
                for _ in 0..count {
                    copies.push(reader.copy(values.next()?)?);
                }
                let placeholder = match reader.byte()? {
                    0 => None,
                    1 => Some(reader.copy(values.next()?)?),
                    _ => return None,
                };
                Prepared::Varied(Variants::new(dimensions, copies, placeholder))
            }
            _ => return None,
        };
        (reader.0.is_empty() && values.next().is_none()).then_some(prepared)
    }

    /// One copy's format and size into `record`, its bytes into `values`.
    fn copy(record: &mut Vec<u8>, values: &mut Vec<Vec<u8>>, copy: &Encoded) -> Option<()> {
        record.push(Self::media(copy.media)?);
        Self::size(record, copy.dimensions);
        values.push(copy.bytes.clone());
        Some(())
    }

    fn size(record: &mut Vec<u8>, dimensions: Dimensions) {
        record.extend_from_slice(&dimensions.width.to_le_bytes());
        record.extend_from_slice(&dimensions.height.to_le_bytes());
    }

    /// The byte a format is written as: only the formats the work makes.
    const fn media(media: MediaType) -> Option<u8> {
        match media {
            MediaType::Avif => Some(0),
            MediaType::Jpeg => Some(1),
            MediaType::Png => Some(2),
            _ => None,
        }
    }
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (&first, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(first)
    }

    fn word(&mut self) -> Option<u32> {
        let (word, rest) = self.0.split_first_chunk::<4>()?;
        self.0 = rest;
        Some(u32::from_le_bytes(*word))
    }

    fn size(&mut self) -> Option<Dimensions> {
        Some(Dimensions::new(self.word()?, self.word()?))
    }

    /// One copy: its format and size from the record, its bytes from `bytes`.
    fn copy(&mut self, bytes: Vec<u8>) -> Option<Encoded> {
        let media = match self.byte()? {
            0 => MediaType::Avif,
            1 => MediaType::Jpeg,
            2 => MediaType::Png,
            _ => return None,
        };
        Some(Encoded::new(media, self.size()?, bytes))
    }
}
