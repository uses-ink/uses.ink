//! Stand-ins for the host's image work and the blob store.

use engine::image::{Encoded, Kept, Prepared, Source, Variants};
use engine::{BlobMissing, Blobs, Images};
use model::{Diagnostic, Dimensions, Hash, MediaType, Problem};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// A stand-in for the host's image work, reading an image's bytes as what to do: `animated`
/// keeps it, `broken` refuses it, `clear` has transparency (no placeholder); anything else is
/// 40 by 30 pixels, copied as AVIF and JPEG at widths 20 and 40, its bytes the source's with
/// the format and width after them.
#[derive(Debug, Default)]
pub struct Stand;

impl Stand {
    fn copy(source: &Source<'_>, media: MediaType, width: u32) -> Encoded {
        let mut bytes = source.bytes.to_vec();
        bytes.extend(format!(" {} {width}", media.essence()).into_bytes());
        Encoded::new(media, Dimensions::new(width, width * 3 / 4), bytes)
    }

    #[expect(clippy::result_large_err, reason = "a refusal is the diagnostic the engine records")]
    fn prepare(source: &Source<'_>) -> Result<Prepared, Diagnostic> {
        let dimensions = Dimensions::new(40, 30);
        match source.bytes {
            b"animated" => Ok(Prepared::Kept { dimensions, reason: Kept::Animated }),
            b"broken" => Err(Diagnostic::new(Problem::ImageDecodeFailed {
                path: source.file.clone(),
                reason: "broken".to_owned(),
            })),
            bytes => {
                let copies = [MediaType::Avif, MediaType::Jpeg]
                    .into_iter()
                    .flat_map(|media| [20, 40].map(|width| Self::copy(source, media, width)))
                    .collect();
                let placeholder = (bytes != b"clear").then(|| Self::copy(source, MediaType::Png, 4));
                Ok(Prepared::Varied(Variants::new(dimensions, copies, placeholder)))
            }
        }
    }
}

impl Images for Stand {
    type Refusal = Diagnostic;

    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Diagnostic>> {
        sources.iter().map(Self::prepare).collect()
    }
}

/// Blobs by hash.
#[derive(Debug, Default)]
pub struct Store(pub(super) BTreeMap<Hash, Vec<u8>>);

impl Blobs for Store {
    fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing> {
        self.0.get(hash).map(|bytes| Cow::Borrowed(bytes.as_slice())).ok_or(BlobMissing { hash: *hash })
    }
}
