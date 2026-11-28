//! Callout kinds as Obsidian names them: GitHub's five, and Obsidian's aliases for its own.

use model::{CalloutKind, Slug};

/// A callout kind as written after `[!`, in lower case.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Kind<'a>(&'a str);

impl Kind<'_> {
    /// Each alias Obsidian gives a kind, and the kind.
    const ALIASES: &'static [(&'static str, &'static str)] = &[
        ("summary", "abstract"),
        ("tldr", "abstract"),
        ("hint", "tip"),
        ("check", "success"),
        ("done", "success"),
        ("help", "question"),
        ("faq", "question"),
        ("attention", "warning"),
        ("fail", "failure"),
        ("missing", "failure"),
        ("error", "danger"),
        ("cite", "quote"),
    ];
}

impl<'a> From<&'a str> for Kind<'a> {
    fn from(written: &'a str) -> Self {
        Self(written)
    }
}

impl From<Kind<'_>> for Option<CalloutKind> {
    /// The kind, for GitHub's five and every alias; `None` for any other name, which keeps its
    /// own name as the markdown format reads it.
    fn from(Kind(written): Kind<'_>) -> Self {
        let named = Kind::ALIASES.iter().find(|(alias, _)| *alias == written).map_or(written, |&(_, kind)| kind);
        Some(match named {
            "note" => CalloutKind::Note,
            "tip" => CalloutKind::Tip,
            "important" => CalloutKind::Important,
            "warning" => CalloutKind::Warning,
            "caution" => CalloutKind::Caution,
            _ if named != written => CalloutKind::Other(Slug::parse(named).ok()?),
            _ => return None,
        })
    }
}
