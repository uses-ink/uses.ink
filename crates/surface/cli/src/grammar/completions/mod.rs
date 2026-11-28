//! `ink completions <SHELL>`: a completion script generated from the command tree.
//!
//! The scripts are static: they complete commands, aliases and flags, and the values of enum
//! flags (`--source`, `--color`, `--host-mode`). Completing a decision code or a path from the
//! folder is a dynamic completion (`clap_complete`'s `CompleteEnv`) and is not wired yet.

use clap::{Command, ValueEnum};
use clap_complete::Generator;

/// A shell a script can be printed for. Its own enum rather than `clap_complete::Shell`, because
/// nushell's generator is a separate crate and so cannot be a variant of that one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Shell {
    /// bash.
    Bash,
    /// elvish.
    Elvish,
    /// fish.
    Fish,
    /// nushell.
    Nushell,
    /// PowerShell.
    #[value(name = "powershell")]
    PowerShell,
    /// zsh.
    Zsh,
}

impl Shell {
    /// The name the script is installed under in the program's own words.
    pub(crate) const NAME: &'static str = "ink";

    /// The script for `command`, which is the decorated command of the tree.
    pub(crate) fn script(self, command: &Command) -> String {
        match self {
            Self::Bash => Self::generate(clap_complete::Shell::Bash, command),
            Self::Elvish => Self::generate(clap_complete::Shell::Elvish, command),
            Self::Fish => Self::generate(clap_complete::Shell::Fish, command),
            Self::PowerShell => Self::generate(clap_complete::Shell::PowerShell, command),
            Self::Zsh => Self::generate(clap_complete::Shell::Zsh, command),
            Self::Nushell => Self::generate(clap_complete_nushell::Nushell, command),
        }
    }

    /// What `generator` writes for `command`.
    fn generate<G: Generator>(generator: G, command: &Command) -> String {
        let mut bytes = Vec::new();
        clap_complete::generate(generator, &mut command.clone(), Self::NAME, &mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The shell's name as `--help` shows it.
    pub(crate) fn name(self) -> String {
        self.to_possible_value().map_or_else(String::new, |value| value.get_name().to_owned())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::grammar::tree::Tree;

    #[test]
    fn every_shell_completes_the_commands_and_the_aliases() {
        let command = Tree::ink().command();
        for shell in Shell::value_variants() {
            let script = shell.script(&command);
            assert!(script.contains("rollback"), "{}", shell.name());
            assert!(script.contains("completions"), "{}", shell.name());
        }
    }

    #[test]
    fn the_aliases_b_and_c_are_completed() {
        let script = Shell::Fish.script(&Tree::ink().command());
        assert!(script.contains("-a \"b\"") || script.contains("-a 'b'") || script.contains(" b "), "{script}");
    }
}
