//! Links in a commit's tree, resolved against the tree itself, never against the disk.

use std::collections::{BTreeMap, VecDeque};

use gix::ObjectId;
use gix::bstr::{BStr, BString, ByteSlice as _};
use model::ContentPath;

/// What a path of the tree is.
#[derive(Clone, Debug)]
pub(super) enum Node {
    /// A file: its blob.
    File(ObjectId),
    /// A link: the text it holds.
    Link(BString),
    /// A folder that is read.
    Folder,
    /// A folder that is not: a submodule, or the build's own output.
    Other,
}

/// Where a link leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Landing {
    /// A file of the tree, by its blob.
    File(ObjectId),
    /// A folder of the tree, the root among them.
    Folder,
}

/// Why a link leads nowhere in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unresolved {
    /// It leaves the tree: an absolute target, or more `..` than it has folders above it.
    Outside,
    /// It names a path the tree does not hold, goes through a file, or links on past
    /// [`Links::HOPS`] links, as a loop does.
    Broken,
}

/// Every path of a tree, asked where links lead.
#[derive(Clone, Copy, Debug)]
pub(super) struct Links<'a>(pub(super) &'a BTreeMap<String, Node>);

impl Links<'_> {
    /// Links followed before a chain counts as a loop, as Linux's `ELOOP` does.
    const HOPS: usize = 40;

    /// Where the link at `path`, holding `target`, leads: resolved part by part, so a link
    /// through a linked folder is followed as the system would follow it.
    pub(super) fn land(self, path: &ContentPath, target: &BStr) -> Result<Landing, Unresolved> {
        let mut at: Vec<&str> = path.segments().collect();
        at.pop();
        let mut pending: VecDeque<&BStr> = VecDeque::new();
        Self::push(&mut pending, target)?;
        let mut hops = 0;
        while let Some(part) = pending.pop_front() {
            match part.as_bytes() {
                b"" | b"." => {}
                b".." => {
                    at.pop().ok_or(Unresolved::Outside)?;
                }
                name => {
                    let name = name.to_str().map_err(|_| Unresolved::Broken)?;
                    at.push(name);
                    match self.0.get(&at.join("/")) {
                        Some(Node::Link(next)) => {
                            hops += 1;
                            if hops > Self::HOPS {
                                return Err(Unresolved::Broken);
                            }
                            at.pop();
                            Self::push(&mut pending, next.as_ref())?;
                        }
                        Some(Node::File(_)) if pending.iter().any(|rest| !matches!(rest.as_bytes(), b"" | b".")) => {
                            return Err(Unresolved::Broken);
                        }
                        Some(_) => {}
                        None => return Err(Unresolved::Broken),
                    }
                }
            }
        }
        if at.is_empty() {
            return Ok(Landing::Folder);
        }
        match self.0.get(&at.join("/")) {
            Some(Node::File(id)) => Ok(Landing::File(*id)),
            Some(Node::Folder | Node::Other) => Ok(Landing::Folder),
            Some(Node::Link(_)) | None => Err(Unresolved::Broken),
        }
    }

    /// Puts `target`'s parts before what is still to resolve; an absolute target leaves the tree.
    fn push<'t>(pending: &mut VecDeque<&'t BStr>, target: &'t BStr) -> Result<(), Unresolved> {
        if target.starts_with(b"/") {
            return Err(Unresolved::Outside);
        }
        for part in target.split(|&byte| byte == b'/').rev() {
            pending.push_front(part.as_bstr());
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn tree() -> BTreeMap<String, Node> {
        let id = ObjectId::null(gix::hash::Kind::Sha1);
        [
            ("a.md", Node::File(id)),
            ("notes", Node::Folder),
            ("notes/b.md", Node::File(id)),
            ("up", Node::Link("../a.md".into())),
            ("notes/up", Node::Link("../a.md".into())),
            ("via", Node::Link("alias/b.md".into())),
            ("alias", Node::Link("notes".into())),
            ("loop", Node::Link("loop".into())),
        ]
        .into_iter()
        .map(|(path, node)| (path.to_owned(), node))
        .collect()
    }

    fn land(path: &str, target: &str) -> Result<Landing, Unresolved> {
        let tree = tree();
        Links(&tree).land(&ContentPath::parse(path).unwrap(), target.into())
    }

    #[test]
    fn links_land_where_the_system_would() {
        let id = ObjectId::null(gix::hash::Kind::Sha1);
        assert_eq!(land("notes/up", "../a.md"), Ok(Landing::File(id)));
        assert_eq!(land("x", "notes"), Ok(Landing::Folder));
        assert_eq!(land("x", "via"), Ok(Landing::File(id)), "through a linked folder");
        assert_eq!(land("notes/x", ".."), Ok(Landing::Folder), "the root is a folder");
        assert_eq!(land("x", "../a.md"), Err(Unresolved::Outside));
        assert_eq!(land("x", "/etc/passwd"), Err(Unresolved::Outside));
        assert_eq!(land("x", "missing.md"), Err(Unresolved::Broken));
        assert_eq!(land("x", "a.md/b"), Err(Unresolved::Broken), "a file is no folder");
        assert_eq!(land("x", "loop"), Err(Unresolved::Broken));
    }
}
