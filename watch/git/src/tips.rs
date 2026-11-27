//! Which of a remote's references a revision names.

use std::fmt;

use gix::ObjectId;
use gix::protocol::handshake::Ref;

/// The remote has nothing for the revision asked.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Missing {
    /// No revision was asked and the remote's `HEAD` points at a branch with no commit yet.
    Unborn,
    /// No revision was asked and the remote advertises no `HEAD`.
    NoHead,
    /// No branch, tag or commit of this name.
    Revision {
        /// The revision as written.
        written: String,
    },
}

impl fmt::Display for Missing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unborn => out.write_str("the remote's default branch has no commit yet"),
            Self::NoHead => out.write_str("the remote advertises no default branch"),
            Self::Revision { written } => write!(out, "the remote has no branch, tag or commit `{written}`"),
        }
    }
}

impl std::error::Error for Missing {}

/// A remote's advertised references.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tips<'a>(pub(crate) &'a [Ref]);

impl Tips<'_> {
    /// The commit a reference of this full name points at, tags peeled.
    fn named(&self, name: &str) -> Option<ObjectId> {
        self.0.iter().find_map(|tip| match tip {
            Ref::Direct { full_ref_name, object }
            | Ref::Peeled { full_ref_name, object, .. }
            | Ref::Symbolic { full_ref_name, object, .. }
                if full_ref_name.as_slice() == name.as_bytes() =>
            {
                Some(*object)
            }
            _ => None,
        })
    }

    /// The commit `revision` names, or the default branch's when none is named: a branch, then a
    /// tag, then a full reference name, then a commit id written out, which names itself.
    pub(crate) fn head(&self, revision: Option<&str>) -> Result<ObjectId, Missing> {
        let Some(written) = revision else {
            let unborn = self.0.iter().any(|tip| matches!(tip, Ref::Unborn { .. }));
            return self.named("HEAD").ok_or(if unborn { Missing::Unborn } else { Missing::NoHead });
        };
        let found = [format!("refs/heads/{written}"), format!("refs/tags/{written}"), written.to_owned()]
            .iter()
            .find_map(|name| self.named(name));
        found
            .or_else(|| ObjectId::from_hex(written.as_bytes()).ok())
            .ok_or_else(|| Missing::Revision { written: written.to_owned() })
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

    use super::*;

    fn id(digit: char) -> ObjectId {
        ObjectId::from_hex(digit.to_string().repeat(40).as_bytes()).unwrap()
    }

    fn tips() -> Vec<Ref> {
        vec![
            Ref::Symbolic {
                full_ref_name: "HEAD".into(),
                target: "refs/heads/main".into(),
                tag: None,
                object: id('a'),
            },
            Ref::Direct { full_ref_name: "refs/heads/main".into(), object: id('a') },
            Ref::Direct { full_ref_name: "refs/heads/next".into(), object: id('b') },
            Ref::Peeled { full_ref_name: "refs/tags/v1".into(), tag: id('c'), object: id('d') },
            Ref::Direct { full_ref_name: "refs/heads/v1".into(), object: id('e') },
        ]
    }

    #[test]
    fn names_the_default_branch_a_branch_or_a_peeled_tag() {
        let tips = tips();
        let tips = Tips(&tips);
        assert_eq!(tips.head(None).unwrap(), id('a'));
        assert_eq!(tips.head(Some("next")).unwrap(), id('b'));
        assert_eq!(tips.head(Some("refs/tags/v1")).unwrap(), id('d'));
        assert_eq!(tips.head(Some("v1")).unwrap(), id('e'), "a branch is looked for before a tag");
        assert_eq!(tips.head(Some(&"f".repeat(40))).unwrap(), id('f'), "a commit id names itself");
        assert_eq!(tips.head(Some("gone")), Err(Missing::Revision { written: "gone".into() }));
    }

    #[test]
    fn says_why_there_is_no_default_branch() {
        let unborn = [Ref::Unborn { full_ref_name: "HEAD".into(), target: "refs/heads/main".into() }];
        assert_eq!(Tips(&unborn).head(None), Err(Missing::Unborn));
        assert_eq!(Tips(&[]).head(None), Err(Missing::NoHead));
    }
}
