//! What a build produces and says about itself: the manifest, the build record, diagnostics, the
//! frontmatter format surface and the git capability.

pub(crate) mod diagnostic;
pub(crate) mod format;
pub mod git;
pub(crate) mod manifest;
pub mod record;
pub(crate) mod written;
