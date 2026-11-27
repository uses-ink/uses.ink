//! `ink man`: the manual page, a roff document generated from the command tree.
//!
//! `clap_mangen` writes the name, synopsis, options and commands from the decorated command; the
//! examples, the exit codes and the environment come from the tree, since clap has no place for
//! them.

use clap_mangen::Man;
use clap_mangen::roff::{Roff, bold, roman};

use crate::tree::{Entry, Tree};

/// The manual of a tree.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Manual<'a> {
    tree: &'a Tree,
}

impl<'a> Manual<'a> {
    /// The manual of `tree`.
    pub(crate) const fn of(tree: &'a Tree) -> Self {
        Self { tree }
    }

    /// The page as roff.
    pub(crate) fn render(&self) -> std::io::Result<String> {
        let man = Man::new(self.tree.command());
        let mut bytes: Vec<u8> = Vec::new();
        man.render_title(&mut bytes)?;
        man.render_name_section(&mut bytes)?;
        man.render_synopsis_section(&mut bytes)?;
        man.render_description_section(&mut bytes)?;
        man.render_options_section(&mut bytes)?;
        man.render_subcommands_section(&mut bytes)?;
        self.sections().to_writer(&mut bytes)?;
        man.render_version_section(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The sections clap has no place for.
    fn sections(&self) -> Roff {
        let mut roff = Roff::new();
        roff.control("SH", ["EXAMPLES"]);
        for example in self.every(|entry| entry.examples.iter().map(|example| (example.line, example.about)).collect())
        {
            roff.control("TP", []).text([bold(example.0)]).text([roman(example.1)]);
        }
        roff.control("SH", ["EXIT STATUS"]);
        for ending in self.tree.endings {
            roff.control("TP", []).text([bold(ending.exit.code().to_string())]).text([roman(ending.about)]);
        }
        roff.control("SH", ["ENVIRONMENT"]);
        for variable in self.tree.env.iter().chain(self.entries().flat_map(|entry| entry.env)) {
            roff.control("TP", []).text([bold(variable.name)]).text([roman(variable.about)]);
        }
        roff
    }

    /// Every entry, the commands of a group after the group.
    fn entries(&self) -> impl Iterator<Item = &'a Entry> {
        self.tree.entries.iter().flat_map(|entry| std::iter::once(entry).chain(entry.children))
    }

    /// What `pick` gives for the root, then for every entry.
    fn every(&self, pick: impl Fn(&Entry) -> Vec<(&'static str, &'static str)>) -> Vec<(&'static str, &'static str)> {
        let root = self.tree.examples.iter().map(|example| (example.line, example.about));
        root.chain(self.entries().flat_map(|entry| pick(entry))).collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_page_names_the_commands_the_codes_and_the_variables() {
        let page = Manual::of(Tree::ink()).render().unwrap_or_default();
        for word in ["ink\\-build", "EXIT STATUS", "INK_COLOR", "rollback"] {
            assert!(page.contains(word), "{word}");
        }
    }
}
