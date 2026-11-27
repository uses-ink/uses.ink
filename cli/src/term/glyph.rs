//! The two glyph sets, `unicode` and `ascii` (`docs/cli.md` section 5), chosen once per run.

use super::role::Role;

/// A mark that has no word of its own: a rail, a branch, a state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Glyph {
    /// A branch that has more after it.
    Branch,
    /// The last branch.
    Last,
    /// A rail running down.
    Rail,
    /// The top of a rail.
    Top,
    /// Live, published.
    Live,
    /// Finished, chosen.
    Done,
    /// Found nothing, skipped.
    Gone,
    /// Private, not published.
    Private,
    /// A rung that was outranked.
    Outranked,
    /// Added.
    Added,
    /// Removed.
    Removed,
    /// A value that moved or changed.
    Changed,
    /// From one value to another.
    To,
    /// Text that was cut.
    Truncation,
    /// A filled cell of a progress bar.
    BarFull,
    /// The leading edge of a progress bar.
    BarHead,
    /// An empty cell of a progress bar.
    BarRest,
}

/// Which characters the run draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Glyphs {
    Unicode,
    Ascii,
}

impl Glyphs {
    /// How `glyph` is written in this set.
    pub(crate) const fn get(self, glyph: Glyph) -> &'static str {
        match (self, glyph) {
            (Self::Unicode, Glyph::Branch) => "├─",
            (Self::Ascii, Glyph::Branch) => "|-",
            (Self::Unicode, Glyph::Last) => "╰─",
            (Self::Ascii, Glyph::Last) => "`-",
            (Self::Unicode, Glyph::Rail) => "│",
            (Self::Ascii, Glyph::Rail) => "|",
            (Self::Unicode, Glyph::Top) => "╭─",
            (Self::Ascii, Glyph::Top) => ".-",
            (Self::Unicode, Glyph::Live) => "●",
            (Self::Ascii, Glyph::Live) => "*",
            (Self::Unicode, Glyph::Done) => "✓",
            (Self::Ascii, Glyph::Done) => "ok",
            (Self::Unicode, Glyph::Gone) => "·",
            (Self::Ascii, Glyph::Gone) => ".",
            (Self::Unicode, Glyph::Private) => "○",
            (Self::Ascii, Glyph::Private) => "o",
            (Self::Unicode, Glyph::Outranked) => "–",
            (_, Glyph::Added) => "+",
            (Self::Ascii, Glyph::Outranked | Glyph::BarRest) | (_, Glyph::Removed) => "-",
            (_, Glyph::Changed) => "~",
            (Self::Unicode, Glyph::To) => "→",
            (Self::Ascii, Glyph::To) => "->",
            (Self::Unicode, Glyph::Truncation) => "…",
            (Self::Ascii, Glyph::Truncation) => "...",
            (Self::Unicode, Glyph::BarFull) => "━",
            (Self::Ascii, Glyph::BarFull) => "#",
            (Self::Unicode, Glyph::BarHead) => "╸",
            (Self::Ascii, Glyph::BarHead) => ">",
            (Self::Unicode, Glyph::BarRest) => "─",
        }
    }
}

impl Glyph {
    /// The role the glyph is drawn in.
    pub(crate) const fn role(self) -> Role {
        match self {
            Self::Branch
            | Self::Last
            | Self::Rail
            | Self::Top
            | Self::Gone
            | Self::Private
            | Self::Outranked
            | Self::Truncation => Role::Quiet,
            Self::Live | Self::Done | Self::Added => Role::Good,
            Self::Removed => Role::Bad,
            Self::Changed => Role::Warn,
            Self::To => Role::Plain,
            Self::BarFull | Self::BarHead | Self::BarRest => Role::Work,
        }
    }
}
