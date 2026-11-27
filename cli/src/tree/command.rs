//! The tree written onto the clap command the argument types derive.

use clap::{Command, CommandFactory};

use super::{Entry, Tree};
use crate::args::Cli;

impl Entry {
    /// `command` with this entry's names and wording, and its children's, applied.
    fn decorate(&self, command: Command) -> Command {
        let mut command = command.about(self.about).visible_aliases(self.aliases.iter().copied());
        if !self.detail.is_empty() {
            command = command.long_about(format!("{}\n\n{}", self.about, self.detail));
        }
        for child in self.children {
            command = command.mut_subcommand(child.name, |sub| child.decorate(sub));
        }
        command
    }
}

impl Tree {
    /// The command clap parses with: the derived one, decorated. Parsing, the completions and the
    /// manual read this; help is written by [`crate::help::Help`] from this and the tree.
    pub(crate) fn command(&self) -> Command {
        let mut command = Cli::command().about(self.about).bin_name("ink");
        for entry in self.entries {
            command = command.mut_subcommand(entry.name, |sub| entry.decorate(sub));
        }
        command
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The names of the subcommands clap holds, hidden ones included.
    fn held(command: &Command) -> Vec<String> {
        command.get_subcommands().map(|sub| sub.get_name().to_owned()).collect()
    }

    #[test]
    fn the_tree_and_the_derived_command_hold_the_same_commands() {
        let tree = Tree::ink();
        let command = tree.command();
        let mut in_tree: Vec<String> = tree.entries.iter().map(|entry| entry.name.to_owned()).collect();
        let mut in_clap = held(&command);
        in_tree.sort();
        in_clap.sort();
        assert_eq!(in_tree, in_clap);
        for entry in tree.entries.iter().filter(|entry| !entry.children.is_empty()) {
            let sub = command.find_subcommand(entry.name).map(held);
            let mut want: Vec<String> = entry.children.iter().map(|child| child.name.to_owned()).collect();
            let mut have = sub.unwrap_or_default();
            want.sort();
            have.sort();
            assert_eq!(want, have, "{}", entry.name);
        }
    }

    #[test]
    fn the_command_passes_clap_s_own_checks() {
        Tree::ink().command().debug_assert();
    }

    #[test]
    fn every_command_is_in_one_group_and_says_what_it_does() {
        for entry in Tree::ink().entries {
            assert!(!entry.about.is_empty() && !entry.about.ends_with('.'), "{}", entry.name);
        }
    }
}
