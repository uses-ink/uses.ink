//! Colour roles and the theme that maps each to one style (`docs/cli.md` section 5).

use anstyle::{AnsiColor, Style};

/// What a piece of text means. The set is closed: a new role is a design change, and code asks for
/// a role, never a colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// Section headings, the subject of a tree, the first line of a diagnostic.
    Heading,
    /// Anything the person can type: commands, flags, the `fix:` label.
    Literal,
    /// Good news: finished verbs, `+`, published.
    Good,
    /// Look at this: warnings, a change ink itself caused.
    Warn,
    /// Stop: errors, `-`, failures.
    Bad,
    /// Something in progress.
    Work,
    /// A name to find again: deploy ids, pins.
    Id,
    /// A placeholder for what the person types in: `<FOLDER>` in a usage line.
    Value,
    /// Where something is: file and line, the snippet gutter.
    Place,
    /// A catalog code in a list.
    Code,
    /// An address to open: a URL, underlined so it reads as a link.
    Link,
    /// A number, a size or a length of time: the values of a settings listing.
    Figure,
    /// The second half of a story: evidence, rails, units.
    Quiet,
    /// Everything else.
    Plain,
}

/// Whether a role's style is also bold. Error and warning labels and the closing verb are strong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Strength {
    Normal,
    Strong,
}

/// The map from role to style, one per run. The palette is the terminal's own: the eight normal
/// ANSI colours, bold and faint, and no background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Theme {
    /// Every role is the default style, so no escape code is written.
    Plain,
    /// The roles of the design.
    Ansi,
}

impl Theme {
    /// The style of `role`.
    pub(crate) const fn style(self, role: Role, strength: Strength) -> Style {
        let base = Style::new();
        let style = match self {
            Self::Plain => return base,
            Self::Ansi => match role {
                Role::Heading => base.bold(),
                Role::Literal => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan))).bold(),
                Role::Good => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green))),
                Role::Warn => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow))),
                Role::Bad => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red))),
                Role::Work | Role::Value => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan))),
                Role::Id | Role::Figure => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Magenta))),
                Role::Place | Role::Code => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Blue))),
                Role::Link => base.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan))).underline(),
                Role::Quiet => base.dimmed(),
                Role::Plain => base,
            },
        };
        match strength {
            Strength::Normal => style,
            Strength::Strong => style.bold(),
        }
    }

    /// Every role, for the golden that prints the table.
    #[cfg(test)]
    pub(crate) const ROLES: [Role; 14] = [
        Role::Heading,
        Role::Literal,
        Role::Good,
        Role::Warn,
        Role::Bad,
        Role::Work,
        Role::Id,
        Role::Value,
        Role::Place,
        Role::Code,
        Role::Link,
        Role::Figure,
        Role::Quiet,
        Role::Plain,
    ];
}
