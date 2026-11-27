//! Raw HTML, taken apart far enough to keep what decision 0027 allows and remove the rest: tags
//! with their attributes, comments and the text between. Nothing here builds a tree or knows the
//! allowlist; the body pairs tags and decides what each becomes.

mod attribute;
mod text;
mod tokens;

pub(crate) use attribute::Attributes;
pub(crate) use text::{Collapsed, Decoded};
pub(crate) use tokens::{Piece, Pieces, Tag};
