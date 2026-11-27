//! The command tree: every command `ink` has, as typed values.
//!
//! One declaration says what a command is called, which group it is listed in, what it does,
//! what to try, which exit codes it can end with and which variables it reads. Four readers sit
//! on it: [`Tree::command`] decorates the clap command the argument types derive (so parsing,
//! the completions and the manual see the same names, aliases and wording), [`crate::help::Help`]
//! writes `ink --help` and per-command help, [`crate::completions::Shell`] prints the scripts and
//! [`crate::man::Manual`] the manual page. A command added to one is in all four or the
//! consistency test fails.

mod command;
mod entries;
mod words;

pub(crate) use words::Located;

use crate::exit::Exit;

/// The help headings options are listed under, spelled once for the argument types and for help.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Heading;

impl Heading {
    /// Where a build writes.
    pub(crate) const OUTPUT: &'static str = "Output";
    /// Where the files are read from.
    pub(crate) const SOURCE: &'static str = "Source";
    /// What is kept between runs.
    pub(crate) const WORK: &'static str = "Work";
    /// Checks of the build itself.
    pub(crate) const VERIFICATION: &'static str = "Verification";
    /// Where a server listens and what it serves.
    pub(crate) const SERVER: &'static str = "Server";
    /// What a server refuses to take.
    pub(crate) const LIMITS: &'static str = "Limits";
    /// Where a server's sign-in mail goes and how it is sent.
    pub(crate) const MAIL: &'static str = "Mail";
    /// How a person signs in to a server and for how long.
    pub(crate) const SIGN_IN: &'static str = "Sign-in";
    /// How a server checks signed requests and enrols keys.
    pub(crate) const KEYS: &'static str = "Keys";
    /// How the loops and lookups behind a server's API run and restart.
    pub(crate) const HOSTING: &'static str = "Hosting";
    /// Which site and which account a remote call is made as.
    pub(crate) const ACCOUNT: &'static str = "Account";
    /// The log: what to say about the run itself, and where.
    pub(crate) const DIAGNOSTICS: &'static str = "Diagnostics";
    /// The options every command takes.
    pub(crate) const GLOBAL: &'static str = "Global options";
}

/// The titled groups the commands are listed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Group {
    /// What you do to the folder in front of you.
    Write,
    /// Why the site looks as it does, and how to change that.
    Understand,
    /// Talking to uses.ink.
    Publish,
    /// Shell and help plumbing.
    Tools,
}

impl Group {
    /// Every group, in the order help lists them.
    pub(crate) const ALL: [Self; 4] = [Self::Write, Self::Understand, Self::Publish, Self::Tools];

    /// The heading help prints.
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Write => "Write",
            Self::Understand => "Understand",
            Self::Publish => "Publish",
            Self::Tools => "Tools",
        }
    }
}

/// One line to type and what it does.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Example {
    /// The line, as typed.
    pub(crate) line: &'static str,
    /// What it does.
    pub(crate) about: &'static str,
}

/// One exit code a command can end with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ending {
    /// The code.
    pub(crate) exit: Exit,
    /// What it means for this command.
    pub(crate) about: &'static str,
}

/// One environment variable and what it decides.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Variable {
    /// Its name.
    pub(crate) name: &'static str,
    /// What it decides.
    pub(crate) about: &'static str,
}

/// One command of the tree, a group of commands when it has [`Entry::children`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    /// The name typed.
    pub(crate) name: &'static str,
    /// Other names, shown in help.
    pub(crate) aliases: &'static [&'static str],
    /// The group listing it, for a command of the root.
    pub(crate) group: Group,
    /// Whether help leaves it out: a spelling kept for the documents that use it.
    pub(crate) hidden: bool,
    /// One sentence.
    pub(crate) about: &'static str,
    /// A short paragraph, empty when the name says it all.
    pub(crate) detail: &'static str,
    /// What to try.
    pub(crate) examples: &'static [Example],
    /// The codes it can end with.
    pub(crate) endings: &'static [Ending],
    /// The variables it reads of its own.
    pub(crate) env: &'static [Variable],
    /// The commands under it, for a group.
    pub(crate) children: &'static [Entry],
}

impl Entry {
    /// The name and its aliases as help lists them: `check, c`.
    pub(crate) fn names(&self) -> String {
        std::iter::once(self.name).chain(self.aliases.iter().copied()).collect::<Vec<_>>().join(", ")
    }

    /// Whether `word` is its name or one of its aliases.
    pub(crate) fn answers(&self, word: &str) -> bool {
        self.name == word || self.aliases.contains(&word)
    }
}

/// The whole tree: the root's own help and the commands under it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tree {
    /// What `ink` is, in one sentence.
    pub(crate) about: &'static str,
    /// What to try first.
    pub(crate) examples: &'static [Example],
    /// The variables every command reads.
    pub(crate) env: &'static [Variable],
    /// The closed set of exit codes and the two that are the shell's and Rust's own.
    pub(crate) endings: &'static [Ending],
    /// The commands.
    pub(crate) entries: &'static [Entry],
}

impl Tree {
    /// The tree of `ink`.
    pub(crate) const fn ink() -> &'static Self {
        &entries::INK
    }

    /// The commands of `group`, in the order help lists them, hidden ones left out.
    pub(crate) fn listed(&self, group: Group) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(move |entry| entry.group == group && !entry.hidden)
    }
}
