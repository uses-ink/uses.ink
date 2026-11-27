//! Words as typed on the command line, found in the tree.

use super::{Entry, Tree};

/// A place in the tree: the root, or a command under it.
#[derive(Clone, Debug)]
pub(crate) struct Located<'a> {
    /// The tree.
    pub(crate) tree: &'a Tree,
    /// The commands from the root, by their names; empty for the root.
    pub(crate) path: Vec<&'static str>,
    /// The command, or `None` for the root.
    pub(crate) entry: Option<&'a Entry>,
}

impl<'a> Located<'a> {
    /// The root.
    pub(crate) const fn root(tree: &'a Tree) -> Self {
        Self { tree, path: Vec::new(), entry: None }
    }

    /// The deepest command `words` name in order, as far as they name any: a word that is no
    /// command (a flag's value, a path) is skipped, a flag is skipped, and the walk stops at `--`.
    pub(crate) fn walk<S: AsRef<str>>(tree: &'a Tree, words: &[S]) -> Self {
        let mut at = Self::root(tree);
        for word in words.iter().map(AsRef::as_ref).take_while(|word| *word != "--") {
            if word.starts_with('-') {
                continue;
            }
            if let Some(next) = at.enter(word) {
                at = next;
            }
        }
        at
    }

    /// The command `word` names here, when it names one.
    pub(crate) fn enter(&self, word: &str) -> Option<Self> {
        let below: &'a [Entry] = self.entry.map_or(self.tree.entries, |entry| entry.children);
        let found = below.iter().find(|entry| entry.answers(word))?;
        let mut path = self.path.clone();
        path.push(found.name);
        Some(Self { tree: self.tree, path, entry: Some(found) })
    }
}
