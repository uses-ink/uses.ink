use model::Bytes;

/// What a clone may cost: an operator's limit that a site may only lower (decision 0044).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchLimits {
    /// The most the clone may take on disk: `source.budget.bytes`.
    pub bytes: Bytes,
    /// The most objects it may hold: `source.budget.objects`.
    pub objects: u64,
}

impl Default for FetchLimits {
    /// 1 GiB, as architecture.md section 13 says, and five million objects, which is more than
    /// the Linux kernel's history holds.
    fn default() -> Self {
        Self { bytes: Bytes(1 << 30), objects: 5_000_000 }
    }
}
