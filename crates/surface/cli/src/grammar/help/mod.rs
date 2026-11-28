//! `ink --help` and the help of each command, written by ink from the command tree and the
//! clap command it decorates (decision 0061, `docs/cli.md` section 4).
//!
//! Clap parses; this writes. The layout is the document's: about, usage, the groups or option
//! groups, the examples, the exit codes, the environment and a pointer to the global options.
//! Short help (`-h`) stops after the options; long help (`--help`) adds the paragraph, the
//! examples, the exit codes and the environment.

mod catalog;
mod page;
mod paint;

use clap::{Arg, Command};

pub(crate) use catalog::Refusal;
use page::{Page, Row};
pub(crate) use paint::{Face, Paint};

use crate::grammar::tree::{Group, Heading, Located, Tree, Variable};

/// How much help is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Depth {
    /// `-h`: the options and no more.
    Short,
    /// `--help`: the paragraph, the examples, the exit codes and the environment too.
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

/// The widest a page is wrapped, whatever the terminal offers.
const WIDEST: usize = 100;

/// The width help is wrapped at when nothing says otherwise.
const USUAL: usize = 80;

/// The width the environment gives: `INK_WIDTH` (`0` for no limit), then `COLUMNS`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Columns;

impl Columns {
    /// The number of columns, when one is given.
    pub(crate) fn of(env: &impl crate::settings::Environment) -> Option<usize> {
        let number = |name: &str| env.get(name).and_then(|text| text.trim().parse::<usize>().ok());
        number("INK_WIDTH").map(|width| if width == 0 { WIDEST } else { width }).or_else(|| number("COLUMNS"))
    }
}

/// A command's place in the tree and in the clap command, and how much help is asked for.
#[derive(Clone, Copy, Debug)]
struct View<'a> {
    at: &'a Located<'a>,
    entry: &'a crate::grammar::tree::Entry,
    command: &'a Command,
    depth: Depth,
}

/// Writes help for a tree, with `command` its decorated clap command, in the faces of `paint`.
#[derive(Debug)]
pub(crate) struct Help<'a, P> {
    tree: &'a Tree,
    command: Command,
    paint: P,
    width: usize,
}

impl<'a, P: Paint> Help<'a, P> {
    /// Help for `tree`, wrapped at `columns` (at most 100; 80 when there is no number).
    pub(crate) fn new(tree: &'a Tree, paint: P, columns: Option<usize>) -> Self {
        let mut command = tree.command();
        command.build();
        Self { tree, command, paint, width: columns.unwrap_or(USUAL).clamp(40, WIDEST) }
    }

    /// The text of the help of the command `at` names, or of the root.
    pub(crate) fn render(&self, at: &Located<'_>, depth: Depth) -> String {
        let mut page = Page::new(&self.paint, self.width);
        let command = at.path.iter().try_fold(&self.command, |command, name| command.find_subcommand(name));
        match (at.entry, command) {
            (Some(entry), Some(command)) => self.command_page(&mut page, &View { at, entry, command, depth }),
            _ => self.root_page(&mut page, depth),
        }
        page.finish()
    }

    /// What `ink help <words>` prints: the root, a command, `exit`, `environment`, or a code.
    pub(crate) fn topic(&self, words: &[String]) -> Result<String, Refusal> {
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
    pub(crate) fn exit(&self) -> String {
        let mut page = Page::new(&self.paint, self.width);
        let rows: Vec<Row> =
            self.tree.endings.iter().map(|ending| Row::keyed(ending.exit.code().to_string(), ending.about)).collect();
        page.table("Exit codes:", &rows);
        page.paragraph(
            "130 is the shell's code for a command that was interrupted and 101 is Rust's for a panic. Neither is in the set above; both can end any command.",
        );
        page.finish()
    }

    /// The table of environment variables, `ink help environment`: the root's, then each
    /// command's of its own.
    pub(crate) fn environment(&self) -> String {
        let mut page = Page::new(&self.paint, self.width);
        let mut seen: Vec<&str> = Vec::new();
        let mut rows = Vec::new();
        let own = self.tree.entries.iter().chain(self.tree.entries.iter().flat_map(|entry| entry.children));
        for variable in self.tree.env.iter().chain(own.flat_map(|entry| entry.env)) {
            if !seen.contains(&variable.name) {
                seen.push(variable.name);
                rows.push(Row::typed(variable.name, variable.about));
            }
        }
        page.table("Environment:", &rows);
        page.finish()
    }

    /// What a diagnostic, notice or decision code means, `ink help <code>`.
    pub(crate) fn code(&self, word: &str) -> Result<String, Refusal> {
        let mut page = Page::new(&self.paint, self.width);
        catalog::Meaning::of(word)?.write(&mut page);
        Ok(page.finish())
    }

    /// The root: the groups of commands, the global options, the examples.
    fn root_page(&self, page: &mut Page<'_, P>, depth: Depth) {
        page.paragraph(&format!("{}.", self.tree.about));
        page.line(&format!("Usage: {} [OPTIONS] <COMMAND>", self.paint.paint(Face::Literal, "ink")));
        for group in Group::ALL {
            let rows: Vec<Row> = self.tree.listed(group).map(Self::entry_row).collect();
            page.table(group.title(), &rows);
        }
        page.table(&format!("{}:", Heading::GLOBAL), &self.globals());
        if depth == Depth::Long {
            Self::examples(page, self.tree.examples);
            Self::environment_block(page, self.tree.env);
        }
        page.line("See `ink help <command>` for one command, and `ink help exit` for exit codes.");
    }

    /// A command: a group of commands, or one with options.
    fn command_page(&self, page: &mut Page<'_, P>, view: &View<'_>) {
        let View { at, entry, command, depth } = *view;
        page.paragraph(&format!("{}.", entry.about));
        if depth == Depth::Long && !entry.detail.is_empty() {
            page.paragraph(entry.detail);
        }
        page.line(&self.usage(at, command));
        if !entry.children.is_empty() {
            let rows: Vec<Row> = entry.children.iter().map(Self::entry_row).collect();
            page.table("Commands", &rows);
        }
        Self::option_groups(page, command, depth);
        if depth == Depth::Long {
            Self::examples(page, entry.examples);
            let rows: Vec<Row> =
                entry.endings.iter().map(|ending| Row::keyed(ending.exit.code().to_string(), ending.about)).collect();
            page.table("Exit codes:", &rows);
            Self::environment_block(page, entry.env);
        }
        page.line(&self.global_pointer(!entry.children.is_empty()));
    }

    /// `Usage: ink build [OPTIONS]`.
    fn usage(&self, at: &Located<'_>, command: &Command) -> String {
        let mut words = vec!["ink".to_owned()];
        words.extend(at.path.iter().map(|name| (*name).to_owned()));
        let own: Vec<&Arg> = Self::shown(command).collect();
        if own.iter().any(|arg| !arg.is_positional()) {
            words.push("[OPTIONS]".to_owned());
        }
        for arg in own.iter().filter(|arg| arg.is_positional()) {
            let name = arg
                .get_value_names()
                .and_then(|names| names.first())
                .map_or_else(|| arg.get_id().as_str().to_uppercase(), ToString::to_string);
            let many = arg.get_num_args().is_some_and(|range| range.max_values() > 1);
            let dots = if many { "..." } else { "" };
            words.push(if arg.is_required_set() { format!("<{name}>{dots}") } else { format!("[{name}]{dots}") });
        }
        if command.has_subcommands() {
            words.push("<COMMAND>".to_owned());
        }
        words.split_first().map_or_else(String::new, |(first, rest)| {
            let mut line = format!("Usage: {}", self.paint.paint(Face::Literal, first));
            for word in rest {
                line.push(' ');
                line.push_str(word);
            }
            line
        })
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
    fn option_groups(page: &mut Page<'_, P>, command: &Command, depth: Depth) {
        let mut headings: Vec<(&str, Vec<Row>)> = Vec::new();
        for arg in Self::shown(command) {
            let heading = if arg.is_positional() { "Arguments" } else { arg.get_help_heading().unwrap_or("Options") };
            let row = Self::arg_row(arg, depth);
            match headings.iter_mut().find(|(name, _)| *name == heading) {
                Some((_, rows)) => rows.push(row),
                None => headings.push((heading, vec![row])),
            }
        }
        headings.sort_by_key(|(name, _)| *name != "Arguments");
        for (heading, rows) in headings {
            page.table(heading, &rows);
        }
    }

    /// The global options as rows: those the root's command declares, then help and version.
    fn globals(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .command
            .get_arguments()
            .filter(|arg| arg.is_global_set())
            .map(|arg| Self::arg_row(arg, Depth::Short))
            .collect();
        rows.push(Row::typed("-h, --help", "Print help (-h is short, --help is long)"));
        rows.push(Row::typed("-V, --version", "Print the version"));
        rows
    }

    /// `Global options: -C, --json, ... (see ink --help)`.
    fn global_pointer(&self, group: bool) -> String {
        if group {
            return "Global options: as in `ink --help`; exit codes: see `ink help exit`".to_owned();
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
        format!("Global options: {} (see `ink --help`)", names.join(", "))
    }

    /// An `Examples:` table.
    fn examples(page: &mut Page<'_, P>, examples: &[crate::grammar::tree::Example]) {
        if !examples.is_empty() {
            let rows: Vec<Row> = examples.iter().map(|example| Row::typed(example.line, example.about)).collect();
            page.table("Examples:", &rows);
        }
    }

    /// An `Environment:` table.
    fn environment_block(page: &mut Page<'_, P>, variables: &[Variable]) {
        if !variables.is_empty() {
            let rows: Vec<Row> = variables.iter().map(|variable| Row::typed(variable.name, variable.about)).collect();
            page.table("Environment:", &rows);
        }
    }

    /// A command as a row of a listing: its names and what it does.
    fn entry_row(entry: &crate::grammar::tree::Entry) -> Row {
        Row::typed(entry.names(), entry.about.to_owned())
    }

    /// An option as a row: how it is typed, and what it does with its default and variable.
    fn arg_row(arg: &Arg, depth: Depth) -> Row {
        let value = || {
            arg.get_value_names()
                .and_then(|names| names.first())
                .map_or_else(|| arg.get_id().as_str().to_uppercase(), ToString::to_string)
        };
        let counted = matches!(arg.get_action(), clap::ArgAction::Count);
        let left = if arg.is_positional() {
            format!("<{}>", value())
        } else {
            let names = match (arg.get_short(), arg.get_long()) {
                (Some(short), Some(long)) => format!("-{short}, --{long}"),
                (Some(short), None) => format!("-{short}"),
                (None, Some(long)) => format!("    --{long}"),
                (None, None) => String::new(),
            };
            let takes = arg.get_action().takes_values();
            format!(
                "{names}{}{}",
                if takes { format!(" <{}>", value()) } else { String::new() },
                if counted { "..." } else { "" }
            )
        };
        let text = match depth {
            Depth::Long => arg.get_long_help().or_else(|| arg.get_help()),
            Depth::Short => arg.get_help(),
        };
        let mut notes: Vec<String> = text.map(ToString::to_string).into_iter().collect();
        let defaults: Vec<String> =
            arg.get_default_values().iter().map(|value| value.to_string_lossy().into_owned()).collect();
        if arg.get_action().takes_values()
            && !defaults.is_empty()
            && !arg.is_hide_default_value_set()
            && !notes.iter().any(|note| note.contains("[default"))
        {
            notes.push(format!("[default: {}]", defaults.join(", ")));
        }
        if let Some(variable) = arg.get_env() {
            notes.push(format!("[env: {}]", variable.to_string_lossy()));
        }
        Row::typed(left, notes.join(" "))
    }
}
