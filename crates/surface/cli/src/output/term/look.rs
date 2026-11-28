//! What a run's terminal is, decided once and per stream (`docs/cli.md` section 13): colour,
//! glyphs, width and whether progress is drawn live.
//!
//! Nothing here reads the process. [`Surroundings`] is the process as the caller reports it, so a
//! test builds one by hand and a golden never changes with the machine it runs on.

use std::collections::BTreeMap;

use clap::ValueEnum;

use crate::grammar::args::When;

use super::glyph::Glyphs;
use super::role::Theme;

/// How many columns a line may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Width {
    /// No limit: nothing is cut.
    Unlimited,
    /// At most this many columns.
    Columns(usize),
}

impl Width {
    /// The columns a line may take; all of them when not limited.
    pub(crate) const fn room(self) -> usize {
        match self {
            Self::Unlimited => usize::MAX,
            Self::Columns(columns) => columns,
        }
    }
}

/// How the output of one stream looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Look {
    pub(crate) colour: bool,
    pub(crate) glyphs: Glyphs,
    pub(crate) width: Width,
    /// The stream is a terminal, which is where a header and a live line are drawn.
    pub(crate) tty: bool,
}

impl Look {
    /// Plain text, unicode glyphs, no limit, not a terminal: what a captured stream looks like.
    pub(crate) const PLAIN: Self = Self { colour: false, glyphs: Glyphs::Unicode, width: Width::Unlimited, tty: false };

    /// The theme of this look.
    pub(crate) const fn theme(self) -> Theme {
        if self.colour { Theme::Ansi } else { Theme::Plain }
    }
}

/// What the person asked for on the command line.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Asked {
    /// `--color`.
    pub(crate) color: Option<When>,
    /// `--no-progress`.
    pub(crate) no_progress: bool,
    /// `[ui] color` of the config file: under the flag and `INK_COLOR`, over the terminal.
    pub(crate) config: Option<When>,
}

impl Asked {
    /// What the raw arguments say of colour and progress, for a command line clap refuses before
    /// any flag is read. `--color` takes its value after a space or an equals sign.
    pub(crate) fn scan(args: &[std::ffi::OsString]) -> Self {
        let mut preferences = Self::default();
        let mut words = args.iter().skip(1).filter_map(|arg| arg.to_str());
        while let Some(word) = words.next() {
            match word.split_once('=') {
                Some(("--color", value)) => preferences.color = When::from_str(value, true).ok(),
                _ if word == "--color" => preferences.color = words.next().and_then(|v| When::from_str(v, true).ok()),
                _ if word == "--no-progress" => preferences.no_progress = true,
                _ => {}
            }
        }
        preferences
    }
}

/// The process, as the caller reports it: the environment variables `ink` reads and which streams
/// are terminals. [`Surroundings::empty`] is a process with none of them and no terminal, which is
/// what a program that captures both streams has; `main` builds the real one.
#[derive(Clone, Debug, Default)]
pub struct Surroundings {
    /// Whether this describes a real process; an empty one is read as a capture.
    declared: bool,
    vars: BTreeMap<String, String>,
    out: Stream,
    err: Stream,
}

/// One stream of the process.
#[derive(Clone, Copy, Debug, Default)]
struct Stream {
    tty: bool,
    columns: Option<usize>,
}

impl Stream {
    /// What the stream is, asked of the system.
    fn of<S: std::io::IsTerminal>(stream: &S, size: Option<terminal_size::Width>) -> Self {
        Self { tty: stream.is_terminal(), columns: size.map(|width| usize::from(width.0)) }
    }
}

impl Surroundings {
    /// The variables `ink` reads (`ink help environment`); no other is looked at.
    const READ: [&'static str; 12] = [
        "INK_COLOR",
        "INK_GLYPHS",
        "INK_NO_PROGRESS",
        "INK_WIDTH",
        "NO_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "TERM",
        "COLUMNS",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
    ];

    /// A process with no variables and no terminal: output is plain text with unicode glyphs, no
    /// width limit and no live progress, unless a flag says otherwise.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The running process: its variables and its terminals.
    pub fn process() -> Self {
        let vars = Self::READ
            .iter()
            .filter_map(|name| std::env::var(name).ok().map(|value| ((*name).to_owned(), value)))
            .collect();
        let (stdout, stderr) = (std::io::stdout(), std::io::stderr());
        Self {
            declared: true,
            vars,
            out: Stream::of(&stdout, terminal_size::terminal_size_of(&stdout).map(|(width, _)| width)),
            err: Stream::of(&stderr, terminal_size::terminal_size_of(&stderr).map(|(width, _)| width)),
        }
    }

    /// A process with these variables and these streams, as a program that captures its output
    /// and wants it to look as it would on a terminal says what it is.
    pub fn declared(vars: &[(&str, &str)], out_tty: bool, err_tty: bool) -> Self {
        let vars = vars.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect();
        let stream = |tty| Stream { tty, columns: None };
        Self { declared: true, vars, out: stream(out_tty), err: stream(err_tty) }
    }

    /// The value of a variable, when it is set and not empty.
    fn var(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str).filter(|value| !value.is_empty())
    }

    /// Whether the environment turns colour on or off when nothing else said: the first match of
    /// `NO_COLOR`, `CLICOLOR_FORCE`, `CLICOLOR` and `TERM=dumb`, else whether the stream is a
    /// terminal.
    fn colours(&self, tty: bool) -> bool {
        if self.var("NO_COLOR").is_some() {
            false
        } else if self.var("CLICOLOR_FORCE").is_some_and(|value| value != "0") {
            true
        } else if self.var("CLICOLOR") == Some("0") || self.dumb() {
            false
        } else {
            tty
        }
    }

    /// Whether `TERM` says the terminal cannot draw.
    fn dumb(&self) -> bool {
        self.var("TERM") == Some("dumb")
    }

    /// Which glyphs: `INK_GLYPHS`, else ascii on a dumb or Linux console or a locale that is not
    /// UTF-8.
    fn glyphs(&self) -> Glyphs {
        match self.var("INK_GLYPHS") {
            Some("ascii") => return Glyphs::Ascii,
            Some("unicode") => return Glyphs::Unicode,
            _ => {}
        }
        let locale = ["LC_ALL", "LC_CTYPE", "LANG"].iter().find_map(|name| self.var(name));
        let utf8 = locale.is_some_and(|locale| {
            let locale = locale.to_ascii_lowercase();
            locale.contains("utf-8") || locale.contains("utf8")
        });
        if utf8 && !matches!(self.var("TERM"), Some("dumb" | "linux")) { Glyphs::Unicode } else { Glyphs::Ascii }
    }

    /// The width: `INK_WIDTH` (`0` for no limit), else on a terminal its size, else `COLUMNS`;
    /// off a terminal nothing is cut.
    fn width(&self, stream: Stream) -> Width {
        let declared = self.var("INK_WIDTH").and_then(|text| text.parse::<usize>().ok());
        match declared {
            Some(0) => Width::Unlimited,
            Some(columns) => Width::Columns(columns),
            None if stream.tty => stream
                .columns
                .or_else(|| self.var("COLUMNS").and_then(|text| text.parse().ok()))
                .map_or(Width::Unlimited, Width::Columns),
            None => Width::Unlimited,
        }
    }

    /// How one stream looks: `--color` first, then `INK_COLOR`, then the config file, then the
    /// environment.
    fn look(&self, stream: Stream, (when, config): (Option<When>, Option<When>)) -> Look {
        let variable = self.var("INK_COLOR").and_then(|text| When::from_str(text, true).ok());
        let colour = match when.or(variable).or(config) {
            Some(When::Always) => true,
            Some(When::Never) => false,
            Some(When::Auto) | None => self.colours(stream.tty),
        };
        Look { colour, glyphs: self.glyphs(), width: self.width(stream), tty: stream.tty }
    }
}

/// What a run decided about both streams and its progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Terminal {
    pub(crate) out: Look,
    pub(crate) err: Look,
    /// Progress is drawn on one line that is redrawn: stderr is a terminal that can, and nothing
    /// asked for log lines.
    pub(crate) live: bool,
}

impl Terminal {
    /// The decisions for `surroundings` and what the command line asked for. A capture has no
    /// environment to ask, so only `--color always` changes it.
    pub(crate) fn decide(surroundings: &Surroundings, preferences: Asked) -> Self {
        if !surroundings.declared {
            let look = Look { colour: preferences.color == Some(When::Always), ..Look::PLAIN };
            return Self { out: look, err: look, live: false };
        }
        let live = surroundings.err.tty
            && !surroundings.dumb()
            && !preferences.no_progress
            && surroundings.var("INK_NO_PROGRESS").is_none_or(|value| value == "0");
        Self {
            out: surroundings.look(surroundings.out, (preferences.color, preferences.config)),
            err: surroundings.look(surroundings.err, (preferences.color, preferences.config)),
            live,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn err(vars: &[(&str, &str)], tty: bool, color: Option<When>) -> Terminal {
        Terminal::decide(&Surroundings::declared(vars, tty, tty), Asked { color, ..Asked::default() })
    }

    #[test]
    fn colour_follows_the_first_rule_that_matches() {
        let utf8 = ("LANG", "en_US.UTF-8");
        assert!(err(&[utf8], true, None).err.colour, "a terminal is coloured");
        assert!(!err(&[utf8], false, None).err.colour, "a pipe is not");
        assert!(!err(&[utf8, ("NO_COLOR", "1")], true, None).err.colour);
        assert!(err(&[utf8, ("NO_COLOR", "")], true, None).err.colour, "an empty NO_COLOR is not set");
        assert!(err(&[utf8, ("CLICOLOR_FORCE", "1")], false, None).err.colour);
        assert!(!err(&[utf8, ("CLICOLOR_FORCE", "0")], false, None).err.colour);
        assert!(!err(&[utf8, ("CLICOLOR", "0")], true, None).err.colour);
        assert!(!err(&[utf8, ("TERM", "dumb")], true, None).err.colour);
        assert!(err(&[utf8, ("NO_COLOR", "1"), ("INK_COLOR", "always")], false, None).err.colour);
        assert!(!err(&[utf8, ("INK_COLOR", "always")], true, Some(When::Never)).err.colour, "the flag wins");
    }

    #[test]
    fn the_variables_that_say_nothing_leave_the_terminal_to_decide() {
        let utf8 = ("LANG", "en_US.UTF-8");
        assert!(err(&[utf8, ("CLICOLOR", "1")], true, None).err.colour);
        assert!(err(&[utf8, ("TERM", "xterm-256color")], true, None).err.colour);
        assert!(!err(&[utf8, ("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")], true, None).err.colour, "NO_COLOR wins");
        assert!(err(&[utf8, ("NO_COLOR", "1")], false, Some(When::Always)).err.colour, "the flag decides alone");
        assert!(!err(&[utf8, ("CLICOLOR_FORCE", "1")], true, Some(When::Never)).err.colour);
    }

    #[test]
    fn the_config_file_is_under_the_flag_and_the_variable() {
        let by = |vars: &[(&str, &str)], flag, config| {
            let asked = Asked { color: flag, config, ..Asked::default() };
            Terminal::decide(&Surroundings::declared(vars, false, false), asked).out.colour
        };
        assert!(by(&[], None, Some(When::Always)));
        assert!(!by(&[("INK_COLOR", "never")], None, Some(When::Always)));
        assert!(by(&[("INK_COLOR", "never")], Some(When::Always), Some(When::Never)));
        assert!(!by(&[], None, None));
    }

    #[test]
    fn colour_is_decided_for_each_stream() {
        let terminal = Terminal::decide(&Surroundings::declared(&[], false, true), Asked::default());
        assert!(!terminal.out.colour);
        assert!(terminal.err.colour);
    }

    #[test]
    fn glyphs_fall_back_to_ascii() {
        let glyphs = |vars: &[(&str, &str)]| err(vars, true, None).err.glyphs;
        assert_eq!(glyphs(&[("LANG", "en_US.UTF-8")]), Glyphs::Unicode);
        assert_eq!(glyphs(&[("LANG", "C")]), Glyphs::Ascii);
        assert_eq!(glyphs(&[]), Glyphs::Ascii);
        assert_eq!(glyphs(&[("LANG", "en_US.UTF-8"), ("TERM", "linux")]), Glyphs::Ascii);
        assert_eq!(glyphs(&[("LANG", "C"), ("INK_GLYPHS", "unicode")]), Glyphs::Unicode);
        assert_eq!(glyphs(&[("LANG", "en_US.UTF-8"), ("INK_GLYPHS", "ascii")]), Glyphs::Ascii);
    }

    #[test]
    fn width_is_cut_only_when_asked_or_on_a_terminal() {
        let width = |vars: &[(&str, &str)], tty| err(vars, tty, None).err.width;
        assert_eq!(width(&[("COLUMNS", "80")], false), Width::Unlimited);
        assert_eq!(width(&[("COLUMNS", "80")], true), Width::Columns(80));
        assert_eq!(width(&[("INK_WIDTH", "40")], false), Width::Columns(40));
        assert_eq!(width(&[("INK_WIDTH", "0"), ("COLUMNS", "80")], true), Width::Unlimited);
    }

    #[test]
    fn progress_is_live_only_on_a_terminal_that_can_draw() {
        let live = |vars: &[(&str, &str)], tty, no_progress| {
            Terminal::decide(&Surroundings::declared(vars, tty, tty), Asked { no_progress, ..Asked::default() }).live
        };
        assert!(live(&[], true, false));
        assert!(!live(&[], false, false));
        assert!(!live(&[], true, true));
        assert!(!live(&[("TERM", "dumb")], true, false));
        assert!(!live(&[("INK_NO_PROGRESS", "1")], true, false));
    }

    #[test]
    fn a_capture_is_plain_unless_colour_is_forced() {
        let plain = Terminal::decide(&Surroundings::empty(), Asked::default());
        assert_eq!(plain.err, Look::PLAIN);
        assert!(!plain.live);
        let forced = Terminal::decide(&Surroundings::empty(), Asked { color: Some(When::Always), ..Asked::default() });
        assert!(forced.out.colour);
    }
}
