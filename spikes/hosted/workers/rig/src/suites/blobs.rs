//! The blob port's suite, on an R2 bucket.

use blob_conformance::Case;
use blob_r2::{Limits, R2};
use worker::{Bucket, Env, Result};

/// The bucket binding of the rig.
pub(crate) struct Blobs<'a> {
    pub(crate) env: &'a Env,
}

impl Blobs<'_> {
    /// The listing pages the suite runs with: three keys, so the listing case crosses pages.
    const PAGE: u32 = 3;

    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        Case::ALL.iter().map(|case| case.name()).collect()
    }

    /// Deletes every object of the bucket.
    pub(crate) async fn clear(bucket: &Bucket) -> Result<()> {
        loop {
            let page = bucket.list().limit(1000).execute().await?;
            let keys: Vec<String> = page.objects().iter().map(worker::Object::key).collect();
            if keys.is_empty() {
                return Ok(());
            }
            bucket.delete_multiple(keys).await?;
        }
    }

    /// Empties the bucket and runs the case; false if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<bool> {
        let Some(case) = Case::named(name) else { return Ok(false) };
        let bucket = self.env.bucket("BUCKET")?;
        Self::clear(&bucket).await?;
        let limits = Limits::new(Limits::PART, Self::PAGE).map_err(|refused| worker::Error::RustError(refused.to_string()))?;
        case.run(&R2::with(bucket, limits)).await;
        Ok(true)
    }
}
