//! How help is coloured: the seam to the terminal layer.
//!
//! Help asks for a [`Face`], never for a colour. The terminal layer (`term`, Part 1 of the CLI
//! redesign) implements [`Paint`] with its theme and its per-stream colour decision; until it is
//! wired in, [`Plain`] writes no escapes and [`Ansi`] writes the three styles help needs with the
//! eight-colour palette and bold and faint of `docs/cli.md` section 5.

use std::ffi::OsString;

use clap::ValueEnum;
use clap::builder::styling::{AnsiColor, Effects, Style};

use crate::args::When;
use crate::settings::Environment;

/// What a piece of help text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Face {
    /// A heading: bold.
    Heading,
    /// Anything you can type: cyan, bold.
    Literal,
}

/// Writes text in a [`Face`].
pub(crate) trait Paint {
    /// `text`, styled.
    fn paint(&self, face: Face, text: &str) -> String;
}

/// No styling at all: a pipe, `NO_COLOR`, a test.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Plain;

impl Paint for Plain {
    fn paint(&self, _: Face, text: &str) -> String {
        text.to_owned()
    }
}

/// ANSI escapes for the three faces.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Ansi;

impl Ansi {
    /// The style of a face.
    fn style(face: Face) -> Style {
        match face {
            Face::Heading => Style::new().effects(Effects::BOLD),
            Face::Literal => Style::new().effects(Effects::BOLD).fg_color(Some(AnsiColor::Cyan.into())),
        }
    }
}

impl Paint for Ansi {
    fn paint(&self, face: Face, text: &str) -> String {
        let style = Self::style(face);
        format!("{style}{text}{style:#}")
    }
}

/// Which stream a text is written to: colour is decided for each one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stream {
    /// Standard output.
    Out,
    /// Standard error.
    Err,
}

impl Stream {
    /// Whether the process's own stream is a terminal.
    pub(crate) fn is_terminal(self) -> bool {
        use std::io::IsTerminal;
        match self {
            Self::Out => std::io::stdout().is_terminal(),
            Self::Err => std::io::stderr().is_terminal(),
        }
    }
}

/// Whether a stream shows colour, by the usual rules.
///
/// `--color always` and `never` decide alone. For `auto` the first rule that applies wins:
/// `NO_COLOR` set and not empty is plain, `CLICOLOR_FORCE` set and not `0` is coloured,
/// `CLICOLOR=0` is plain, `TERM=dumb` is plain, and else the stream's being a terminal decides.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Detect<'a, E> {
    /// The variables.
    env: &'a E,
    /// Whether the stream is a terminal.
    terminal: bool,
}

impl<'a, E: Environment> Detect<'a, E> {
    /// The detection for a stream that is, or is not, a terminal.
    pub(crate) const fn new(env: &'a E, terminal: bool) -> Self {
        Self { env, terminal }
    }

    /// Whether the variable `name` is set to something other than nothing.
    fn given(&self, name: &str) -> Option<String> {
        self.env.get(name).filter(|value| !value.is_empty())
    }

    /// Whether colour shows for `when`.
    pub(crate) fn shows(&self, when: When) -> bool {
        match when {
            When::Always => true,
            When::Never => false,
            When::Auto => {
                if self.given("NO_COLOR").is_some() {
                    false
                } else if self.given("CLICOLOR_FORCE").is_some_and(|value| value != "0") {
                    true
                } else if self.given("CLICOLOR").is_some_and(|value| value == "0")
                    || self.given("TERM").is_some_and(|value| value == "dumb")
                {
                    false
                } else {
                    self.terminal
                }
            }
        }
    }
}

/// The painter a run uses: plain, or ANSI when the stream shows colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Painter {
    /// No escapes.
    Plain,
    /// The three ANSI faces.
    Ansi,
}

impl Painter {
    /// The painter for `when` on a stream: colour per [`Detect`].
    pub(crate) fn of(when: When, detect: &Detect<'_, impl Environment>) -> Self {
        if detect.shows(when) { Self::Ansi } else { Self::Plain }
    }

    /// The painter for what was declared before clap parsed: `--color <when>` or
    /// `--color=<when>` in `args`, else `INK_COLOR`. Help is written when parsing fails, so it
    /// cannot wait for a parsed command line.
    pub(crate) fn declared(args: &[OsString], env: &impl Environment, stream: Stream) -> Self {
        let mut words = args.iter().skip(1).filter_map(|arg| arg.to_str());
        let mut written = None;
        while let Some(word) = words.next() {
            if word == "--" {
                break;
            }
            if word == "--color" {
                written = words.next().map(str::to_owned);
            } else if let Some(value) = word.strip_prefix("--color=") {
                written = Some(value.to_owned());
            }
        }
        let when = written.or_else(|| env.get("INK_COLOR")).and_then(|value| When::from_str(&value, true).ok());
        Self::of(when.unwrap_or_default(), &Detect::new(env, stream.is_terminal()))
    }
}

impl Paint for Painter {
    fn paint(&self, face: Face, text: &str) -> String {
        match self {
            Self::Plain => Plain.paint(face, text),
            Self::Ansi => Ansi.paint(face, text),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    /// A fixed environment.
    struct Fixed(HashMap<&'static str, &'static str>);

    impl Environment for Fixed {
        fn get(&self, name: &str) -> Option<String> {
            self.0.get(name).map(|value| (*value).to_owned())
        }
    }

    fn shows(variables: &[(&'static str, &'static str)], terminal: bool, when: When) -> bool {
        let env = Fixed(variables.iter().copied().collect());
        Detect::new(&env, terminal).shows(when)
    }

    #[test]
    fn auto_follows_the_terminal() {
        assert!(shows(&[], true, When::Auto));
        assert!(!shows(&[], false, When::Auto));
    }

    #[test]
    fn the_flag_decides_alone() {
        assert!(shows(&[("NO_COLOR", "1")], false, When::Always));
        assert!(!shows(&[("CLICOLOR_FORCE", "1")], true, When::Never));
    }

    #[test]
    fn each_variable_is_read_by_its_own_rule() {
        assert!(!shows(&[("NO_COLOR", "1")], true, When::Auto));
        assert!(shows(&[("NO_COLOR", "")], true, When::Auto), "an empty NO_COLOR is not set");
        assert!(shows(&[("CLICOLOR_FORCE", "1")], false, When::Auto));
        assert!(!shows(&[("CLICOLOR_FORCE", "0")], false, When::Auto));
        assert!(!shows(&[("CLICOLOR", "0")], true, When::Auto));
        assert!(shows(&[("CLICOLOR", "1")], true, When::Auto));
        assert!(!shows(&[("TERM", "dumb")], true, When::Auto));
        assert!(shows(&[("TERM", "xterm-256color")], true, When::Auto));
    }

    #[test]
    fn no_color_beats_a_forced_colour() {
        assert!(!shows(&[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")], true, When::Auto));
    }

    #[test]
    fn a_painter_writes_escapes_only_when_colour_shows() {
        let env = Fixed(HashMap::new());
        assert_eq!(Painter::of(When::Auto, &Detect::new(&env, true)), Painter::Ansi);
        assert_eq!(Painter::of(When::Auto, &Detect::new(&env, false)), Painter::Plain);
        assert_eq!(Painter::of(When::Never, &Detect::new(&env, true)), Painter::Plain);
        let paint = Painter::Ansi.paint(Face::Heading, "x");
        assert!(paint.contains('\u{1b}'));
        assert_eq!(Painter::Plain.paint(Face::Heading, "x"), "x");
    }
}
