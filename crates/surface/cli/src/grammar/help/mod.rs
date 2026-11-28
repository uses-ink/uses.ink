//! `ink --help` and the help of each command, written by ink from the command tree and the clap
//! command it decorates (decision 0061, decision 0220, `docs/cli.md` section 4).
//!
//! Clap parses; this writes, as lines of roles through the terminal layer, so help is coloured,
//! wrapped and aligned by the same code as every other block of output. Every page has the same
//! anatomy: what it does in a line, the usage, the examples, the commands, the arguments, the
//! options under their headings, and what to read next. Short help (`-h`) stops after the options;
//! long help (`--help`) adds the examples, the notes, the exit codes, the environment and the
//! pointers.

mod catalog;
#[cfg(test)]
mod golden;
mod page;

use clap::{Arg, Command};

pub(crate) use catalog::Refusal;
use page::{Measure, Sheet, Typed};

use crate::grammar::tree::{Entry, Group, Heading, Located, Tree, Variable};
use crate::output::term::{Defs, Doc, Line, Look, Role, Samples};

/// How much help is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Depth {
    /// `-h`: the options and no more.
    Short,
    /// `--help`: the examples, the notes, the exit codes, the environment and the pointers too.
    Long,
}

impl Depth {
    /// `--help` asks for the long form and `-h` for the short one; the long one wins when both
    /// are given. Anything after `--` is not a flag.
    pub(crate) fn of(args: &[std::ffi::OsString]) -> Self {
        let flags = args.iter().skip(1).take_while(|arg| *arg != "--");
        if flags.into_iter().any(|arg| arg == "--help") { Self::Long } else { Self::Short }
    }
}

/// A command's place in the tree and in the clap command, and how much help is asked for.
#[derive(Clone, Copy, Debug)]
struct View<'a> {
    at: &'a Located<'a>,
    entry: &'a Entry,
    command: &'a Command,
    depth: Depth,
}

/// The note a help text may end with, `[default: x]`, taken off the text so help can set it apart.
#[derive(Clone, Copy, Debug)]
struct Note<'a> {
    /// The text without the note.
    prose: &'a str,
    /// What the note says, without brackets.
    default: Option<&'a str>,
}

impl<'a> Note<'a> {
    /// What `text` says and, when it ends in `[default: ...]` (and perhaps a full stop), the note.
    fn of(text: &'a str) -> Self {
        let trimmed = text.trim_end().trim_end_matches('.');
        match (trimmed.rfind("[default: "), trimmed.ends_with(']')) {
            (Some(at), true) => Self {
                prose: text.get(..at).unwrap_or(text).trim_end(),
                default: trimmed.get(at + "[default: ".len()..trimmed.len() - 1),
            },
            _ => Self { prose: text, default: None },
        }
    }
}

/// Writes help for a tree, with `command` its decorated clap command, in the look it is given.
#[derive(Debug)]
pub(crate) struct Help<'a> {
    tree: &'a Tree,
    command: Command,
    look: Look,
}

impl<'a> Help<'a> {
    /// Help for `tree`, laid out for a stream that looks like `look`: wrapped at its width, between
    /// 40 and 100 columns, and at 80 when it has no limit.
    pub(crate) fn new(tree: &'a Tree, look: Look) -> Self {
        let mut command = tree.command();
        command.build();
        Self { tree, command, look: Measure::of(look) }
    }

    /// The page of the command `at` names, or of the root.
    pub(crate) fn render(&self, at: &Located<'_>, depth: Depth) -> Doc {
        let mut sheet = Sheet::new(self.look);
        let command = at.path.iter().try_fold(&self.command, |command, name| command.find_subcommand(name));
        match (at.entry, command) {
            (Some(entry), Some(command)) => self.command_page(&mut sheet, &View { at, entry, command, depth }),
            _ => self.root_page(&mut sheet, depth),
        }
        sheet.finish()
    }

    /// What `ink help <words>` prints: the root, a command, `exit`, `environment`, or a code.
    pub(crate) fn topic(&self, words: &[String]) -> Result<Doc, Refusal> {
        match words {
            [] => Ok(self.render(&Located::root(self.tree), Depth::Long)),
            [word] if word == "exit" => Ok(self.exit()),
            [word] if word == "environment" => Ok(self.environment()),
            _ => {
                let found = words.iter().try_fold(Located::root(self.tree), |at, word| at.enter(word));
                match (found, words) {
                    (Some(at), _) => Ok(self.render(&at, Depth::Long)),
                    (None, [word]) => self.code(word),
                    (None, _) => Err(Refusal::Topic { words: words.to_vec() }),
                }
            }
        }
    }

    /// The table of exit codes, `ink help exit`.
    pub(crate) fn exit(&self) -> Doc {
        let mut sheet = Sheet::new(self.look);
        let mut defs = Defs::new(2);
        for ending in self.tree.endings {
            defs = defs.row(Line::plain(ending.exit.code().to_string()), Line::marked(Role::Plain, ending.about));
        }
        sheet.section("Exit codes", &defs);
        sheet.paragraph(Line::plain(
            "130 is the shell's code for a command that was interrupted and 101 is Rust's for a panic. Neither is in the set above; both can end any command.",
        ));
        sheet.finish()
    }

    /// The table of environment variables, `ink help environment`: the root's, then each
    /// command's of its own.
    pub(crate) fn environment(&self) -> Doc {
        let mut sheet = Sheet::new(self.look);
        let mut seen: Vec<&str> = Vec::new();
        let mut own: Vec<Variable> = Vec::new();
        let walked = self.tree.walk();
        for variable in self.tree.env.iter().chain(walked.iter().flat_map(|(_, entry)| entry.env)) {
            if !seen.contains(&variable.name) {
                seen.push(variable.name);
                own.push(*variable);
            }
        }
        sheet.section("Environment", &Self::variables(&own));
        sheet.finish()
    }

    /// What a diagnostic, notice or decision code means, `ink help <code>`.
    pub(crate) fn code(&self, word: &str) -> Result<Doc, Refusal> {
        let mut sheet = Sheet::new(self.look);
        catalog::Meaning::of(word)?.write(&mut sheet);
        Ok(sheet.finish())
    }

    /// The root: the groups of commands, the global options, the examples.
    fn root_page(&self, sheet: &mut Sheet, depth: Depth) {
        sheet.paragraph(Line::marked(Role::Plain, &format!("{}.", self.tree.about)));
        sheet.line(Self::usage_line(&(vec!["ink".to_owned()], vec!["[OPTIONS]".to_owned(), "<COMMAND>".to_owned()])));
        if depth == Depth::Long {
            self.examples(sheet, self.tree.examples);
        }
        for group in Group::ALL {
            let mut defs = Defs::new(2);
            for entry in self.tree.listed(group) {
                defs = defs.row(Self::names(entry), Line::marked(Role::Plain, entry.about));
            }
            sheet.section(group.title(), &defs);
        }
        sheet.section(Heading::GLOBAL, &self.globals());
        if depth == Depth::Long {
            sheet.section("Environment", &Self::variables(self.tree.env));
        }
        sheet.paragraph(Line::marked(
            Role::Quiet,
            "Read `ink help <command>` for one command and `ink help exit` for the exit codes.",
        ));
    }

    /// A command: a group of commands, or one with options.
    fn command_page(&self, sheet: &mut Sheet, view: &View<'_>) {
        let View { at, entry, command, depth } = *view;
        sheet.paragraph(Line::marked(Role::Plain, &format!("{}.", entry.about)));
        sheet.line(Self::usage_line(&Self::usage_words(at, command)));
        if !entry.aliases.is_empty() {
            let also: Vec<String> = entry.aliases.iter().map(|alias| format!("ink {alias}")).collect();
            sheet.line(Line::new().with(Role::Quiet, "Also typed as ").with(Role::Literal, also.join(", ")));
        }
        if depth == Depth::Long {
            self.examples(sheet, entry.examples);
        }
        if !entry.children.is_empty() {
            let mut defs = Defs::new(2);
            for child in entry.children {
                defs = defs.row(Self::names(child), Line::marked(Role::Plain, child.about));
            }
            sheet.section("Commands", &defs);
        }
        Self::option_groups(sheet, command, depth);
        if depth == Depth::Long {
            if !entry.detail.is_empty() {
                sheet
                    .section("Notes", &crate::output::term::Paragraph::new(Line::marked(Role::Plain, entry.detail), 2));
            }
            let mut defs = Defs::new(2);
            for ending in entry.endings {
                defs = defs.row(Line::plain(ending.exit.code().to_string()), Line::marked(Role::Plain, ending.about));
            }
            sheet.section("Exit codes", &defs);
            if !entry.env.is_empty() {
                sheet.section("Environment", &Self::variables(entry.env));
            }
        }
        self.foot(sheet, view);
    }

    /// `Usage: ink build [OPTIONS]`: the words of the command typed, the placeholders after them.
    fn usage_line(words: &(Vec<String>, Vec<String>)) -> Line {
        let (words, tail) = words;
        let mut line = Line::new().strong(Role::Heading, "Usage").with(Role::Plain, ": ");
        for (at, word) in words.iter().enumerate() {
            if at > 0 {
                line = line.with(Role::Plain, " ");
            }
            line = line.strong(Role::Literal, word);
        }
        for word in tail {
            line = line.with(Role::Plain, " ").with(Role::Value, word);
        }
        line
    }

    /// The words of the usage line of a command: what is typed, then the placeholders.
    fn usage_words(at: &Located<'_>, command: &Command) -> (Vec<String>, Vec<String>) {
        let own: Vec<&Arg> = Self::shown(command).collect();
        let mut tail: Vec<String> = Vec::new();
        if own.iter().any(|arg| !arg.is_positional()) {
            tail.push("[OPTIONS]".to_owned());
        }
        tail.extend(own.iter().filter(|arg| arg.is_positional()).map(|arg| Self::positional(arg)));
        if command.has_subcommands() {
            tail.push("<COMMAND>".to_owned());
        }
        let mut words = vec!["ink".to_owned()];
        words.extend(at.path.iter().map(|word| (*word).to_owned()));
        (words, tail)
    }

    /// The name of a command and its aliases as a term: `check, c`.
    fn names(entry: &Entry) -> Line {
        Line::new().strong(Role::Literal, entry.names())
    }

    /// `<FOLDER>` for a required positional, `[FOLDER]` for an optional one, `...` for many.
    fn positional(arg: &Arg) -> String {
        let name = Self::value(arg);
        let many = arg.get_num_args().is_some_and(|range| range.max_values() > 1);
        let dots = if many { "..." } else { "" };
        if arg.is_required_set() { format!("<{name}>{dots}") } else { format!("[{name}]{dots}") }
    }

    /// The name of what an argument takes: its value name, or its id in capitals.
    fn value(arg: &Arg) -> String {
        arg.get_value_names()
            .and_then(|names| names.first())
            .map_or_else(|| arg.get_id().as_str().to_uppercase(), ToString::to_string)
    }

    /// The arguments of `command` that help lists: not its own help and version, not hidden and
    /// not a global option.
    fn shown(command: &Command) -> impl Iterator<Item = &Arg> {
        command.get_arguments().filter(|arg| {
            !arg.is_global_set() && !arg.is_hide_set() && !matches!(arg.get_id().as_str(), "help" | "version")
        })
    }

    /// The options of a command under their headings, in the order the headings first appear;
    /// positionals first under `Arguments`.
    fn option_groups(sheet: &mut Sheet, command: &Command, depth: Depth) {
        let mut headings: Vec<(&str, Defs)> = Vec::new();
        for arg in Self::shown(command) {
            let heading = if arg.is_positional() { "Arguments" } else { arg.get_help_heading().unwrap_or("Options") };
            let (term, meaning) = Self::row(arg, depth);
            match headings.iter().position(|(name, _)| *name == heading) {
                Some(at) => {
                    if let Some((_, defs)) = headings.get_mut(at) {
                        *defs = std::mem::take(defs).row(term, meaning);
                    }
                }
                None => headings.push((heading, Defs::new(2).row(term, meaning))),
            }
        }
        headings.sort_by_key(|(name, _)| *name != "Arguments");
        for (heading, defs) in headings {
            sheet.section(heading, &defs);
        }
    }

    /// The global options as rows: those the root's command declares, then help and version.
    fn globals(&self) -> Defs {
        let mut defs = Defs::new(2);
        for arg in self.command.get_arguments().filter(|arg| arg.is_global_set()) {
            let (term, meaning) = Self::row(arg, Depth::Short);
            defs = defs.row(term, meaning);
        }
        defs = defs.row(
            Line::new().with(Role::Literal, "-h, --help"),
            Line::plain("Print help; -h is short and --help is long"),
        );
        defs.row(Line::new().with(Role::Literal, "-V, --version"), Line::plain("Print the version"))
    }

    /// The foot of a command's page: what to read next, where the docs are, where the global
    /// options are, and what the long form adds.
    fn foot(&self, sheet: &mut Sheet, view: &View<'_>) {
        let View { at, entry, depth, .. } = *view;
        let label = |text: &str| Line::new().with(Role::Quiet, text);
        let mut defs = Defs::new(0);
        if depth == Depth::Long {
            if !entry.see.is_empty() {
                let mut line = Line::new();
                for (at, text) in entry.see.iter().enumerate() {
                    if at > 0 {
                        line = line.with(Role::Plain, ", ");
                    }
                    line = line.atom(Role::Literal, text);
                }
                defs = defs.row(label("See also"), line);
            }
            defs = defs.row(label("Docs"), Line::new().atom(Role::Place, Tree::DOCS));
        }
        let names: Vec<String> = self
            .command
            .get_arguments()
            .filter(|arg| arg.is_global_set())
            .map(|arg| {
                arg.get_short()
                    .map_or_else(|| format!("--{}", arg.get_long().unwrap_or_default()), |short| format!("-{short}"))
            })
            .collect();
        let global = Line::new()
            .with(Role::Literal, names.join(", "))
            .with(Role::Plain, " ")
            .atom(Role::Quiet, "(see `ink --help`)");
        defs = defs.row(label("Global"), global);
        if depth == Depth::Short {
            let mut words = vec!["ink"];
            words.extend(at.path.iter().copied());
            words.push("--help");
            let more = Line::new()
                .atom(Role::Literal, words.join(" "))
                .with(Role::Plain, " adds examples, notes and exit codes");
            defs = defs.row(label("More"), more);
        }
        sheet.block(&defs);
    }

    /// An `Examples` section, each line typed in its roles with its comment.
    fn examples(&self, sheet: &mut Sheet, examples: &[crate::grammar::tree::Example]) {
        if examples.is_empty() {
            return;
        }
        let mut samples = Samples::new(2);
        for example in examples {
            samples = samples
                .row(Typed { tree: self.tree, text: example.line }.line(), Line::marked(Role::Plain, example.about));
        }
        sheet.section("Examples", &samples);
    }

    /// Environment variables as terms and meanings.
    fn variables(variables: &[Variable]) -> Defs {
        variables.iter().fold(Defs::new(2), |defs, variable| {
            defs.row(Line::new().with(Role::Literal, variable.name), Line::marked(Role::Plain, variable.about))
        })
    }

    /// An option or positional as a term and its meaning: what you type, and what it does with
    /// its default, its variable and whether it is required after it.
    fn row(arg: &Arg, depth: Depth) -> (Line, Line) {
        (Self::term(arg), Self::meaning(arg, depth))
    }

    /// What you type for an option or positional: its flags and its placeholder.
    fn term(arg: &Arg) -> Line {
        let counted = matches!(arg.get_action(), clap::ArgAction::Count);
        if arg.is_positional() {
            Line::new().with(Role::Value, Self::positional(arg))
        } else {
            let names = match (arg.get_short(), arg.get_long()) {
                (Some(short), Some(long)) => format!("-{short}, --{long}"),
                (Some(short), None) => format!("-{short}"),
                (None, Some(long)) => format!("    --{long}"),
                (None, None) => String::new(),
            };
            let mut term = Line::new().with(Role::Literal, names);
            if arg.get_action().takes_values() {
                term = term.with(Role::Plain, " ").with(Role::Value, format!("<{}>", Self::value(arg)));
            }
            if counted { term.with(Role::Plain, "...") } else { term }
        }
    }

    /// What an option does, then its default, its variable and whether it is required.
    fn meaning(arg: &Arg, depth: Depth) -> Line {
        let text = match depth {
            Depth::Long => arg.get_long_help().or_else(|| arg.get_help()),
            Depth::Short => arg.get_help(),
        };
        let raw = text.map_or_else(String::new, ToString::to_string);
        let note = Note::of(&raw);
        let mut meaning = Line::marked(Role::Plain, note.prose);
        let defaults: Vec<String> =
            arg.get_default_values().iter().map(|value| value.to_string_lossy().into_owned()).collect();
        let shown = match (note.default, defaults.is_empty()) {
            (Some(default), _) => Some(default.to_owned()),
            (None, false) if arg.get_action().takes_values() && !arg.is_hide_default_value_set() => {
                Some(defaults.join(", "))
            }
            _ => None,
        };
        if let Some(default) = shown {
            meaning = meaning.with(Role::Plain, " ").atom(Role::Quiet, format!("[default: {default}]"));
        }
        if let Some(variable) = arg.get_env() {
            meaning =
                meaning.with(Role::Plain, " ").atom(Role::Quiet, format!("[env: {}]", variable.to_string_lossy()));
        }
        if !arg.is_positional() && arg.is_required_set() {
            meaning = meaning.with(Role::Plain, " ").atom(Role::Warn, "[required]");
        }
        meaning
    }
}
