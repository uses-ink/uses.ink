//! Shell quoting. A word that reaches a shell line is a [`Quoted`]; there is no other way to put
//! one there.

use std::fmt;

/// A word in single quotes, as a POSIX shell reads it back exactly: whatever it holds, it is one
/// word with no expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Quoted(String);

impl Quoted {
    pub(crate) fn new(word: impl AsRef<str>) -> Self {
        Self(word.as_ref().to_owned())
    }
}

impl fmt::Display for Quoted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "'{}'", self.0.replace('\'', "'\\''"))
    }
}

/// Words as one shell line: each quoted, joined by spaces.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Line<'a>(pub(crate) &'a [String]);

impl fmt::Display for Line<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut words = self.0.iter();
        if let Some(first) = words.next() {
            write!(out, "{}", Quoted::new(first))?;
        }
        words.try_for_each(|word| write!(out, " {}", Quoted::new(word)))
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    const HOSTILE: &[&str] = &[
        "plain",
        "two words",
        "it's",
        "'",
        "''",
        "$(touch /tmp/farm-pwned)",
        "`id`",
        "a; rm -rf /",
        "a && b",
        "line\nbreak",
        "back\\slash",
        "\"double\"",
        "*",
        "~",
        "-n",
        "--help",
        "é ü",
        "",
        "a\tb",
        "${HOME}",
        "!history",
    ];

    #[test]
    fn a_shell_reads_every_word_back_unchanged() {
        for word in HOSTILE {
            let line = format!("printf %s {}", Quoted::new(word));
            let out = Command::new("sh").arg("-c").arg(&line).output().unwrap();
            assert_eq!(String::from_utf8(out.stdout).unwrap(), *word, "{line}");
        }
    }

    #[test]
    fn a_line_is_one_argument_per_word() {
        let words: Vec<String> = HOSTILE.iter().map(|word| (*word).to_owned()).collect();
        let line = format!("printf '%s\\0' {}", Line(&words));
        let out = Command::new("sh").arg("-c").arg(&line).output().unwrap();
        let back: Vec<&[u8]> = out.stdout.split(|byte| *byte == 0).collect();
        assert_eq!(back.len(), HOSTILE.len() + 1);
        for (read, word) in back.iter().zip(HOSTILE) {
            assert_eq!(*read, word.as_bytes());
        }
    }

    #[test]
    fn the_text_is_what_the_docs_show() {
        assert_eq!(Quoted::new("it's").to_string(), r"'it'\''s'");
        assert_eq!(Quoted::new("").to_string(), "''");
    }
}
