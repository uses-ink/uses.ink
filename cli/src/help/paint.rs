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

/// The painter a run uses: plain, or ANSI when colour was asked for.
///
/// This is the seam. Until the terminal layer decides colour per stream (a terminal, `NO_COLOR`,
/// `CLICOLOR_FORCE`, `TERM=dumb`), only an explicit `always` colours help; `auto` does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Painter {
    /// No escapes.
    Plain,
    /// The three ANSI faces.
    Ansi,
}

impl Painter {
    /// The painter for a decided colour setting.
    pub(crate) const fn of(when: When) -> Self {
        match when {
            When::Always => Self::Ansi,
            When::Auto | When::Never => Self::Plain,
        }
    }

    /// The painter for what was declared before clap parsed: `--color <when>` or
    /// `--color=<when>` in `args`, else `INK_COLOR`. Help is written when parsing fails, so it
    /// cannot wait for a parsed command line.
    pub(crate) fn declared(args: &[OsString], env: &impl Environment) -> Self {
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
        Self::of(when.unwrap_or_default())
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
