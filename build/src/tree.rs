//! The tree an upload sends: every file of a folder by path, hash, size and modification time,
//! and nothing of the bytes (`docs/architecture.md` section 12: send the tree, upload the missing
//! hashes).
//!
//! A tree is canonical: files sorted by path, each path once, written as compact JSON. Its hash is
//! the hash of those bytes, so two folders with the same files and times have the same tree, and a
//! tree stored as a blob under that hash can be read back and checked.

use std::fmt;

use model::{Bytes, ContentPath, Hash, Timestamp};
use serde::{Deserialize, Serialize};

/// What a tree's bytes start with, and the schema it is read under.
const SCHEMA: &str = "uses.tree/v1";

/// One file of a tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leaf {
    /// Where it is in the folder.
    pub path: ContentPath,
    /// The hash of its bytes.
    pub hash: Hash,
    /// How long it is.
    pub size: Bytes,
    /// When the sender's file system says it last changed, when it says.
    pub modified: Option<Timestamp>,
}

/// The files of an uploaded folder, sorted by path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    schema: String,
    files: Vec<Leaf>,
}

/// Why bytes or files are no tree.
#[derive(Debug)]
#[non_exhaustive]
pub enum TreeError {
    /// The bytes are not the JSON of a tree.
    Json(serde_json::Error),
    /// The tree is of another schema.
    Schema {
        /// The schema it names.
        found: String,
    },
    /// A path is in the tree twice.
    Duplicate {
        /// The path.
        path: ContentPath,
    },
    /// The files are not sorted by path.
    Unsorted {
        /// The first path found out of order.
        path: ContentPath,
    },
}

impl fmt::Display for TreeError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(_) => out.write_str("the bytes are not the JSON of a tree"),
            Self::Schema { found } => write!(out, "the tree is of the schema {found:?}, not {SCHEMA:?}"),
            Self::Duplicate { path } => write!(out, "the path {path} is in the tree twice"),
            Self::Unsorted { path } => write!(out, "the path {path} is not in order: a tree is sorted by path"),
        }
    }
}

impl std::error::Error for TreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            _ => None,
        }
    }
}

impl Tree {
    /// The tree of `files`, sorted. A path twice is refused.
    pub fn of(mut files: Vec<Leaf>) -> Result<Self, TreeError> {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let tree = Self { schema: SCHEMA.to_owned(), files };
        tree.check()?;
        Ok(tree)
    }

    /// The tree `bytes` write: the exact canonical form [`Tree::bytes`] makes, so bytes that
    /// are a tree in another order or spelling are refused.
    pub fn read(bytes: &[u8]) -> Result<Self, TreeError> {
        let tree: Self = serde_json::from_slice(bytes).map_err(TreeError::Json)?;
        tree.check()?;
        Ok(tree)
    }

    fn check(&self) -> Result<(), TreeError> {
        if self.schema != SCHEMA {
            return Err(TreeError::Schema { found: self.schema.clone() });
        }
        for pair in self.files.windows(2) {
            if let [before, after] = pair {
                if before.path == after.path {
                    return Err(TreeError::Duplicate { path: after.path.clone() });
                }
                if before.path > after.path {
                    return Err(TreeError::Unsorted { path: after.path.clone() });
                }
            }
        }
        Ok(())
    }

    /// The canonical bytes: compact JSON.
    pub fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The hash of [`Tree::bytes`].
    pub fn hash(&self) -> Hash {
        Hash::of(self.bytes())
    }

    /// The files, sorted by path.
    pub fn files(&self) -> &[Leaf] {
        &self.files
    }

    /// The bytes all the files hold.
    pub fn total(&self) -> Bytes {
        Bytes(self.files.iter().map(|leaf| leaf.size.count()).fold(0, u64::saturating_add))
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

    use super::*;

    fn leaf(path: &str, text: &str) -> Leaf {
        Leaf {
            path: ContentPath::parse(path).unwrap(),
            hash: Hash::of(text),
            size: Bytes(text.len() as u64),
            modified: None,
        }
    }

    #[test]
    fn a_tree_is_sorted_and_reads_back_to_itself() {
        let tree = Tree::of(vec![leaf("b.md", "b"), leaf("a.md", "a")]).unwrap();
        assert_eq!(tree.files().first().map(|leaf| leaf.path.as_str()), Some("a.md"));
        assert_eq!(Tree::read(&tree.bytes()).unwrap(), tree);
        assert_eq!(tree.total(), Bytes(2));
    }

    #[test]
    fn a_path_twice_or_out_of_order_is_refused() {
        assert!(matches!(Tree::of(vec![leaf("a.md", "1"), leaf("a.md", "2")]), Err(TreeError::Duplicate { .. })));
        let tree = Tree { schema: SCHEMA.to_owned(), files: vec![leaf("b.md", "b"), leaf("a.md", "a")] };
        assert!(matches!(Tree::read(&tree.bytes()), Err(TreeError::Unsorted { .. })));
        let other = Tree { schema: "x".to_owned(), files: Vec::new() };
        assert!(matches!(Tree::read(&other.bytes()), Err(TreeError::Schema { .. })));
    }
}
