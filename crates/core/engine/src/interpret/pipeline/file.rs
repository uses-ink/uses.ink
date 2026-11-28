//! The files under the root: which are kept (`file.ignore`), what formats read, and which plain
//! files are published.

pub(crate) mod asset;
pub(crate) mod ignore;
pub(crate) mod name;
pub(crate) mod read;

pub(crate) use asset::Assets;
pub(crate) use ignore::Files;
pub(crate) use read::Read;
