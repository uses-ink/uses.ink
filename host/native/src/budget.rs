//! How much of a folder a build reads (architecture.md section 13).

use model::Bytes;

/// The limits of one read. Each one passed is a typed refusal with the number and the limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Files in the snapshot: `source.budget.files`.
    pub files: u64,
    /// One file: `source.budget.file`.
    pub file: Bytes,
    /// Every file together: `source.budget.bytes`.
    pub total: Bytes,
}

impl Default for Budget {
    /// The v1 limits: 20,000 files, 25 MiB a file, 1 GiB in all.
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        Self { files: 20_000, file: Bytes(25 * MIB), total: Bytes(1024 * MIB) }
    }
}
