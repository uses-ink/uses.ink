//! A migration file as statements.

use std::fmt;

use crate::Statement;

/// The statements of one `.sql` file, split so each runs as a prepared statement inside a batch.
///
/// D1 refuses explicit transactions (`BEGIN`, `COMMIT`), and a trigger's body needs `BEGIN`, so a
/// file that works here works on D1: both are refused with a [`ScriptError`].
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    statements: Vec<Statement>,
}

/// A transaction keyword a script may not use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Keyword {
    /// `BEGIN`.
    Begin,
    /// `COMMIT`.
    Commit,
    /// `ROLLBACK`.
    Rollback,
    /// `SAVEPOINT`.
    Savepoint,
}

impl Keyword {
    /// The keyword `word` spells, in any case.
    fn of(word: &str) -> Option<Self> {
        [
            ("begin", Self::Begin),
            ("commit", Self::Commit),
            ("rollback", Self::Rollback),
            ("savepoint", Self::Savepoint),
        ]
        .into_iter()
        .find(|(name, _)| word.eq_ignore_ascii_case(name))
        .map(|(_, keyword)| keyword)
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Begin => "BEGIN",
            Self::Commit => "COMMIT",
            Self::Rollback => "ROLLBACK",
            Self::Savepoint => "SAVEPOINT",
        })
    }
}

/// Why text is not a script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScriptError {
    /// A string, quoted name or comment that is never closed, opened at this byte.
    Unterminated {
        /// The byte offset of the opening.
        at: usize,
    },
    /// A transaction keyword at this byte.
    Transaction {
        /// Which one.
        keyword: Keyword,
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for ScriptError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unterminated { at } => write!(out, "the string or comment opened at byte {at} is never closed"),
            Self::Transaction { keyword, at } => {
                write!(out, "{keyword} at byte {at}: D1 refuses transactions, and a migration runs in one batch")
            }
        }
    }
}

impl std::error::Error for ScriptError {}

/// A walk over the bytes of a script.
struct Scan {
    text: &'static str,
    at: usize,
    start: usize,
    content: bool,
    word: Option<usize>,
    statements: Vec<Statement>,
}

impl Scan {
    fn byte(&self, at: usize) -> Option<u8> {
        self.text.as_bytes().get(at).copied()
    }

    /// The offset just past the first `close` at or after `from`, for a quoted name or a comment.
    fn skip(&self, from: usize, close: &[u8], opened: usize) -> Result<usize, ScriptError> {
        let rest = self.text.as_bytes().get(from..).unwrap_or_default();
        rest.windows(close.len())
            .position(|window| window == close)
            .map(|found| from + found + close.len())
            .ok_or(ScriptError::Unterminated { at: opened })
    }

    /// Ends the word being read, refusing a transaction keyword.
    fn end_word(&mut self) -> Result<(), ScriptError> {
        if let Some(from) = self.word.take()
            && let Some(word) = self.text.get(from..self.at)
            && let Some(keyword) = Keyword::of(word)
        {
            return Err(ScriptError::Transaction { keyword, at: from });
        }
        Ok(())
    }

    /// Ends the statement being read at the current offset.
    fn finish(&mut self) {
        if self.content
            && let Some(sql) = self.text.get(self.start..self.at)
        {
            self.statements.push(Statement::prepare(sql.trim()));
        }
        self.start = self.at + 1;
        self.content = false;
    }

    fn run(mut self) -> Result<Vec<Statement>, ScriptError> {
        while let Some(byte) = self.byte(self.at) {
            let opened = self.at;
            match byte {
                b'\'' | b'"' | b'`' => {
                    self.end_word()?;
                    self.content = true;
                    // a doubled quote is an escaped quote: it closes and reopens
                    self.at = self.skip(opened + 1, &[byte], opened)?;
                    continue;
                }
                b'[' => {
                    self.end_word()?;
                    self.content = true;
                    self.at = self.skip(opened + 1, b"]", opened)?;
                    continue;
                }
                b'-' if self.byte(opened + 1) == Some(b'-') => {
                    self.end_word()?;
                    self.at = self.text.get(opened..).and_then(|rest| rest.find('\n')).map_or(self.text.len(), |n| opened + n);
                    continue;
                }
                b'/' if self.byte(opened + 1) == Some(b'*') => {
                    self.end_word()?;
                    self.at = self.skip(opened + 2, b"*/", opened)?;
                    continue;
                }
                b';' => {
                    self.end_word()?;
                    self.finish();
                }
                b if b.is_ascii_alphanumeric() || b == b'_' => {
                    self.content = true;
                    self.word.get_or_insert(opened);
                }
                b if b.is_ascii_whitespace() => self.end_word()?,
                _ => {
                    self.end_word()?;
                    self.content = true;
                }
            }
            self.at += 1;
        }
        self.end_word()?;
        self.finish();
        Ok(self.statements)
    }
}

impl Script {
    /// Splits `text` into its statements. A statement is what lies between `;` outside strings,
    /// quoted names and comments; text with nothing but comments between two is no statement.
    pub fn parse(text: &'static str) -> Result<Self, ScriptError> {
        let scan = Scan { text, at: 0, start: 0, content: false, word: None, statements: Vec::new() };
        scan.run().map(|statements| Self { statements })
    }

    /// The statements, in order.
    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn sql(text: &'static str) -> Vec<&'static str> {
        Script::parse(text).unwrap().statements().iter().map(Statement::sql).collect()
    }

    #[test]
    fn splits_at_semicolons_outside_text() {
        let script = "CREATE TABLE a (x TEXT DEFAULT 'a;b');\n-- note; here\nINSERT INTO a VALUES ('it''s');\n/* c; */ SELECT 1";
        assert_eq!(
            sql(script),
            vec!["CREATE TABLE a (x TEXT DEFAULT 'a;b')", "-- note; here\nINSERT INTO a VALUES ('it''s')", "/* c; */ SELECT 1"]
        );
    }

    #[test]
    fn a_comment_alone_is_no_statement() {
        assert_eq!(sql("-- nothing\n;;\n/* still */;"), Vec::<&str>::new());
    }

    #[test]
    fn refuses_transactions_as_d1_does() {
        let refused = |text: &'static str| Script::parse(text).unwrap_err();
        assert_eq!(refused("select 1; BEGIN TRANSACTION;"), ScriptError::Transaction { keyword: Keyword::Begin, at: 10 });
        assert!(matches!(refused("commit"), ScriptError::Transaction { keyword: Keyword::Commit, .. }));
        // a word inside a name or a string is not the keyword
        assert_eq!(sql("SELECT 'begin', commit_at FROM begins"), vec!["SELECT 'begin', commit_at FROM begins"]);
    }

    #[test]
    fn refuses_what_it_cannot_close() {
        assert_eq!(Script::parse("select 'open").unwrap_err(), ScriptError::Unterminated { at: 7 });
        assert_eq!(Script::parse("/* open").unwrap_err(), ScriptError::Unterminated { at: 0 });
    }
}
