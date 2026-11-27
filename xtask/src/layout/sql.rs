//! The rule that keeps SQL text out of the code (decision 0049): queries and schema are built
//! from typed values, and one renderer turns them into text. So no string literal of code starts
//! with a SQL keyword, no method writes raw SQL into a builder, and no `.sql` file is committed.

use std::fmt;

use proc_macro2::{LexError, TokenStream, TokenTree};

use super::workspace::Workspace;
use super::{Problem, Problems, Rule};

/// What a file holds that the rule refuses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Found {
    /// A string literal that starts with a SQL keyword.
    Literal(String),
    /// A call or macro that writes raw SQL.
    Escape(String),
    /// A `.sql` file.
    File,
}

impl fmt::Display for Found {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(word) => {
                write!(out, "holds a string that starts with `{word}`: build the statement with typed values")
            }
            Self::Escape(name) => {
                write!(out, "calls `{name}`, which writes raw SQL: build the expression with typed values")
            }
            Self::File => out.write_str("is a `.sql` file: migrations are Rust values, not files"),
        }
    }
}

/// No SQL text: no literal that starts with a keyword, no raw escape of the builder, no `.sql`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sql;

impl Sql {
    /// The words that start a statement, in upper case as SQL is written.
    const KEYWORDS: &'static [&'static str] = &[
        "SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "DROP", "ALTER", "PRAGMA", "WITH", "BEGIN", "COMMIT",
        "ROLLBACK", "REPLACE",
    ];

    /// The names with which the builder takes text as SQL.
    const ESCAPES: &'static [&'static str] =
        &["cust", "cust_with_values", "cust_with_expr", "cust_with_exprs", "custom_keyword", "raw_sql", "raw_query"];

    /// This file, whose list of keywords is the one place they are written.
    const OWN: &'static str = "xtask/src/layout/sql.rs";

    /// The folders whose `.sql` files are not ours.
    const FOREIGN: &'static [&'static str] = &["spikes", "target"];

    /// What `text` holds that is refused, in the order it appears.
    pub(crate) fn find(text: &str) -> Result<Vec<Found>, LexError> {
        let mut found = Vec::new();
        Self::within(text.parse::<TokenStream>()?, &mut found);
        Ok(found)
    }

    fn within(tokens: TokenStream, found: &mut Vec<Found>) {
        let mut attribute = false;
        for token in tokens {
            // the group after a `#` is an attribute, which holds doc text and lint reasons
            let skipped = attribute;
            // `#![..]` is an inner attribute: the `!` keeps the flag up for the group after it
            attribute = match &token {
                TokenTree::Punct(punct) => punct.as_char() == '#' || (punct.as_char() == '!' && attribute),
                _ => false,
            };
            match token {
                TokenTree::Group(_) if skipped => {}
                TokenTree::Group(group) => Self::within(group.stream(), found),
                TokenTree::Ident(ident) => {
                    let name = ident.to_string();
                    if Self::ESCAPES.contains(&name.as_str()) {
                        found.push(Found::Escape(name));
                    }
                }
                TokenTree::Literal(literal) => {
                    found.extend(Self::keyword(&literal.to_string()).map(|word| Found::Literal(word.to_owned())));
                }
                TokenTree::Punct(_) => {}
            }
        }
    }

    /// The keyword a string literal (as the token prints, quotes and prefix included) starts with.
    fn keyword(literal: &str) -> Option<&'static str> {
        let body = literal.split_once('"')?.1.trim_start();
        let word = body.split(|c: char| !c.is_ascii_alphabetic()).next()?;
        Self::KEYWORDS.iter().copied().find(|keyword| *keyword == word)
    }
}

impl Rule for Sql {
    const NAME: &'static str = "sql";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for path in &workspace.entries {
            let foreign = path
                .components()
                .next()
                .is_some_and(|first| Self::FOREIGN.iter().any(|name| first.as_os_str() == *name));
            if path.extension().is_some_and(|extension| extension == "sql") && !foreign {
                problems.push(Problem::Sql { path: path.clone(), found: Found::File });
            }
        }
        for file in
            workspace.crates.iter().flat_map(|c| &c.files).filter(|file| file.path != std::path::Path::new(Self::OWN))
        {
            match Self::find(&file.text) {
                Ok(found) => {
                    for found in found {
                        problems.push(Problem::Sql { path: file.path.clone(), found });
                    }
                }
                Err(source) => problems.push(Problem::Lexed { path: file.path.clone(), source }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Found, Sql};

    fn find(code: &str) -> Vec<Found> {
        Sql::find(code).unwrap()
    }

    #[test]
    fn a_literal_that_starts_with_a_keyword() {
        assert_eq!(find(r#"let q = "SELECT 1";"#), [Found::Literal("SELECT".into())]);
        assert_eq!(find(r##"let q = r#"  INSERT INTO t"#;"##), [Found::Literal("INSERT".into())]);
        assert_eq!(find(r#"let q = b"DELETE";"#), [Found::Literal("DELETE".into())]);
    }

    #[test]
    fn prose_and_values_that_hold_a_keyword_later_are_not_sql() {
        assert_eq!(find(r#"let q = "x'); DROP TABLE t; --";"#), Vec::<Found>::new());
        assert_eq!(find(r#"let q = "Select a site";"#), Vec::<Found>::new());
        assert_eq!(find(r#"let q = "SELECTED";"#), Vec::<Found>::new());
        assert_eq!(find(r#"let q = "WAL";"#), Vec::<Found>::new());
    }

    #[test]
    fn attributes_hold_docs_not_code() {
        assert_eq!(find(r#"#[doc = "SELECT 1"] fn f() {}"#), Vec::<Found>::new());
        assert_eq!(find(r#"#![expect(clippy::x, reason = "DELETE the row")]"#), Vec::<Found>::new());
        assert_eq!(find(r"/// SELECT the row"), Vec::<Found>::new());
    }

    #[test]
    fn inner_attributes_and_doc_lines_are_attributes_too() {
        assert_eq!(find("//! SELECT the rows\nfn f() {}"), Vec::<Found>::new());
        assert_eq!(find("#![doc = \"DELETE all\"]"), Vec::<Found>::new());
    }

    #[test]
    fn a_literal_in_nested_code_is_found() {
        assert_eq!(find(r#"fn f() { g(vec!["UPDATE t"]); }"#), [Found::Literal("UPDATE".into())]);
    }

    #[test]
    fn the_raw_escapes_of_the_builders() {
        assert_eq!(find(r#"let e = Expr::cust("1");"#), [Found::Escape("cust".into())]);
        assert_eq!(find("let e = raw_sql!(x);"), [Found::Escape("raw_sql".into())]);
        assert_eq!(find("let customer = 1;"), Vec::<Found>::new());
    }

    #[test]
    fn this_rules_own_words_are_not_matched_by_a_lowercase_literal() {
        assert_eq!(find(r#"let q = "select";"#), Vec::<Found>::new());
    }
}
