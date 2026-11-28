//! The gate on help: a golden of every page in plain, colour and narrow forms, generated from the
//! tree by walking all of it, so a command added without a summary or an example fails here, and
//! checks that no option goes undescribed, no example is a line that does not parse, and no line
//! of a page is wider than its page.

use clap::Parser as _;

use super::{Depth, Help};
use crate::grammar::args::Cli;
use crate::grammar::tree::{Located, Tree};
use crate::output::term::{Doc, Finished, Look, Width, Writer};

/// One page and the words that ask for it.
struct Page {
    name: String,
    at: Located<'static>,
}

/// One way a page can look.
struct Form {
    name: &'static str,
    depth: Depth,
    look: Look,
}

impl Form {
    /// The forms of a golden: long help plain, in colour and in 60 columns, and short help plain.
    fn all() -> [Self; 4] {
        let wide = Look { width: Width::Columns(80), ..Look::PLAIN };
        [
            Self { name: "long-plain", depth: Depth::Long, look: wide },
            Self { name: "long-colour", depth: Depth::Long, look: Look { colour: true, ..wide } },
            Self { name: "long-narrow", depth: Depth::Long, look: Look { width: Width::Columns(60), ..Look::PLAIN } },
            Self { name: "short-plain", depth: Depth::Short, look: wide },
        ]
    }

    /// `doc` as this look writes it, an escape written `\e` so a diff reads.
    fn text(&self, doc: Doc) -> String {
        let mut out = Vec::new();
        Writer::new(&mut out, self.look).emit(&Finished(doc)).unwrap();
        String::from_utf8(out).unwrap().replace('\x1b', "\\e")
    }
}

impl Page {
    /// The root and every command of the tree, the commands of a group after it.
    fn all() -> Vec<Self> {
        let tree = Tree::ink();
        let mut pages = vec![Self { name: "ink".to_owned(), at: Located::root(tree) }];
        for (path, _) in tree.walk() {
            let at = path.iter().try_fold(Located::root(tree), |at, word| at.enter(word)).unwrap();
            pages.push(Self { name: format!("ink-{}", path.join("-")), at });
        }
        pages
    }
}

#[test]
fn every_page_of_help_looks_as_expected() {
    let mut cases = Vec::new();
    for form in Form::all() {
        let help = Help::new(Tree::ink(), form.look);
        for page in Page::all() {
            cases.push((format!("{}-{}", page.name, form.name), form.text(help.render(&page.at, form.depth))));
        }
        if form.depth == Depth::Long {
            let topics: [(&str, Doc); 2] = [("exit", help.exit()), ("environment", help.environment())];
            for (name, doc) in topics {
                cases.push((format!("ink-help-{name}-{}", form.name), form.text(doc)));
            }
        }
    }
    fixture::cases("fixture/help", cases);
}

#[test]
fn every_command_says_what_it_does_and_has_an_example_that_parses() {
    let tree = Tree::ink();
    let mut problems: Vec<String> = Vec::new();
    for (path, entry) in tree.walk() {
        let named = path.join(" ");
        if entry.about.is_empty() || entry.about.ends_with('.') {
            problems.push(format!("ink {named} needs a one line summary without a full stop"));
        }
        if entry.hidden {
            continue;
        }
        if entry.examples.is_empty() || entry.examples.len() > 4 {
            problems.push(format!("ink {named} needs one to four examples, it has {}", entry.examples.len()));
        }
        for example in entry.examples {
            if example.about.is_empty() {
                problems.push(format!("ink {named}: `{}` needs a comment", example.line));
            }
            let words = Words(example.line).split();
            if let Err(error) = Cli::try_parse_from(&words) {
                problems.push(format!(
                    "ink {named}: `{}` does not parse: {}",
                    example.line,
                    error.to_string().lines().next().unwrap_or_default()
                ));
                continue;
            }
            let there = Located::walk(tree, words.get(1..).unwrap_or_default());
            if !there.path.starts_with(&path) {
                problems.push(format!("ink {named}: `{}` is about another command", example.line));
            }
        }
        for pointed in entry.see {
            let words: Vec<&str> = pointed.split(' ').skip(1).collect();
            if Located::walk(tree, &words).path.len() != words.len() {
                problems.push(format!("ink {named} points to `{pointed}`, which is no command"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_option_is_described_and_a_default_ends_its_description() {
    let command = Tree::ink().command();
    let mut stack = vec![(String::from("ink"), &command)];
    while let Some((name, command)) = stack.pop() {
        for arg in command.get_arguments().filter(|arg| !arg.is_hide_set()) {
            let id = arg.get_id().as_str();
            if matches!(id, "help" | "version") {
                continue;
            }
            assert!(
                arg.get_help().is_some_and(|help| !help.to_string().trim().is_empty()),
                "{name} {id} has no description"
            );
            let text = arg.get_help().map_or_else(String::new, ToString::to_string);
            let note = super::Note::of(&text);
            assert_eq!(
                text.contains("[default"),
                note.default.is_some(),
                "{name} {id}: a default is the last words of the description, as `[default: x]`"
            );
        }
        stack.extend(command.get_subcommands().map(|sub| (format!("{name} {}", sub.get_name()), sub)));
    }
}

#[test]
fn no_line_of_a_page_is_wider_than_the_page() {
    for width in [60, 80] {
        let look = Look { width: Width::Columns(width), ..Look::PLAIN };
        let help = Help::new(Tree::ink(), look);
        for page in Page::all() {
            for depth in [Depth::Long, Depth::Short] {
                let mut typing = false;
                for (number, line) in help.render(&page.at, depth).lines().iter().enumerate() {
                    let text = line.text();
                    if !text.starts_with(' ') && !text.is_empty() {
                        typing = text == "Examples";
                    }
                    // a command or an address is never broken, so a long one overflows
                    let command = (typing && text.starts_with("  ink ")) || text.contains("https://");
                    let single = text.trim_start().split(' ').count() == 1;
                    assert!(
                        line.width() <= width || single || command,
                        "{} line {number} is {} columns at {width}: {text}",
                        page.name,
                        line.width()
                    );
                    let glyph = text
                        .chars()
                        .find(|c| matches!(c, '\u{2014}' | '\u{2013}' | '\u{2192}' | '\u{2190}' | '\u{21d2}'));
                    assert!(glyph.is_none(), "{} uses {glyph:?} in prose: {text}", page.name);
                }
            }
        }
    }
}

#[test]
fn a_pipe_wraps_at_eighty_and_a_flag_sets_nothing_else() {
    let piped = Help::new(Tree::ink(), Look::PLAIN);
    let root = Located::root(Tree::ink());
    let widest = piped.render(&root, Depth::Long).lines().iter().map(crate::output::term::Line::width).max().unwrap();
    assert!(widest <= 80, "{widest}");
}

#[test]
fn command_code_holds_no_sentence_after_a_sentence() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![base.join("command"), base.join("selfhost")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().map(Result::unwrap) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (number, line) in text.lines().enumerate() {
                    let code = line.trim_start();
                    if code.starts_with("//") || code.starts_with("#[") {
                        continue;
                    }
                    let sentences = line
                        .as_bytes()
                        .windows(4)
                        .any(|w| matches!(w, [b'a'..=b'z' | b'0'..=b'9' | b')' | b'`', b'.', b' ', b'A'..=b'Z']));
                    assert!(
                        !(sentences && line.contains('"')),
                        "{}:{}: output text is typed, one sentence each: {line}",
                        path.display(),
                        number + 1
                    );
                }
            }
        }
    }
}

/// A line to type, split into words the way a shell does: double quotes keep their spaces.
struct Words<'a>(&'a str);

impl Words<'_> {
    fn split(&self) -> Vec<String> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut quoted = false;
        for c in self.0.chars() {
            match (c, quoted) {
                ('"', _) => quoted = !quoted,
                (' ', false) => words.push(std::mem::take(&mut word)),
                _ => word.push(c),
            }
        }
        words.push(word);
        words
    }
}
