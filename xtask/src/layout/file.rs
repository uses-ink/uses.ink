//! The rules over files: banned names anywhere in the tree, unsafe code only where a decision
//! says.

use proc_macro2::{LexError, TokenStream, TokenTree};

use super::workspace::Workspace;
use super::{Problem, Problems, Rule};

/// No file or directory named for a heap of unrelated code.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Banned;

impl Banned {
    /// Names that collect unrelated code.
    const NAMES: &'static [&'static str] = &[
        "util", "utils", "common", "core", "helpers", "misc", "base", "shared", "manager", "handler", "service", "impl",
    ];
}

impl Rule for Banned {
    const NAME: &'static str = "banned";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for path in &workspace.entries {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy()) else { continue };
            let stem = name.split('.').next().unwrap_or(&name);
            if Self::NAMES.contains(&stem) {
                problems.push(Problem::Banned { path: path.clone() });
            }
        }
    }
}

/// Unsafe code only in the files [`Unsafe::ALLOWED`] lists. A file holds it when the keyword,
/// or the lint that guards it, is a token of its code: comments and literals do not count.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Unsafe;

impl Unsafe {
    /// The keyword, spelled in two parts so this file does not hold it.
    pub(crate) const KEYWORD: &'static str = concat!("un", "safe");

    /// The lint that guards the keyword, which only a file allowed unsafe code may name.
    const LINT: &'static str = concat!("un", "safe_code");

    /// The only files that may hold unsafe code: the pen's wasm exports, whose names need the
    /// unsafe `no_mangle` attribute. The workspace denies the lint rather than forbidding it so
    /// these can expect it.
    pub(crate) const ALLOWED: &'static [&'static str] = &["freehand-web/src/lib.rs"];

    /// Whether `text` holds the keyword or names its lint as code.
    fn holds(text: &str) -> Result<bool, LexError> {
        text.parse::<TokenStream>().map(Self::within)
    }

    fn within(tokens: TokenStream) -> bool {
        tokens.into_iter().any(|token| match token {
            TokenTree::Ident(ident) => ident == Self::KEYWORD || ident == Self::LINT,
            TokenTree::Group(group) => Self::within(group.stream()),
            TokenTree::Punct(_) | TokenTree::Literal(_) => false,
        })
    }
}

impl Rule for Unsafe {
    const NAME: &'static str = "unsafe";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for file in workspace.crates.iter().flat_map(|c| &c.files) {
            let allowed = Self::ALLOWED.iter().any(|a| file.path == std::path::Path::new(a));
            match Self::holds(&file.text) {
                Ok(holds) if holds && !allowed => problems.push(Problem::Unsafe { path: file.path.clone() }),
                Ok(_) => {}
                Err(source) => problems.push(Problem::Lexed { path: file.path.clone(), source }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Unsafe;

    fn holds(code: &str) -> bool {
        Unsafe::holds(code).unwrap()
    }

    #[test]
    fn unsafe_in_every_spelling() {
        let k = Unsafe::KEYWORD;
        for code in [
            format!("{k} {{ x }}"),
            format!("{k}{{*p}}"),
            format!("{k}\n{{ }}"),
            format!("pub {k} fn f() {{}}"),
            format!("{k} impl Send for X {{}}"),
            format!("#[{k}(no_mangle)]"),
            format!("#![expect({k}_code)]"),
            format!("let s = \"a\"; {k} {{ x }}"),
            format!("/* a */ {k} {{ x }}"),
            format!("let c = '\"'; {k} {{ x }}"),
            format!("fn f<'a>(x: &'a u8) {{ {k} {{ x }} }}"),
            format!("let s = r#\"a\"#; {k} {{ x }}"),
            format!("let s = \"\\\"\"; {k} {{ x }}"),
        ] {
            assert!(holds(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_as_part_of_a_word() {
        let k = Unsafe::KEYWORD;
        for code in [format!("let {k}ty = 1;"), format!("fn is_{k}() {{}}"), format!("let r#{k}x = 1;")] {
            assert!(!holds(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_in_comments() {
        let k = Unsafe::KEYWORD;
        for code in
            [format!("// {k} {{ }}"), format!("/* {k} {{ }} */"), format!("/* /* */ {k} */"), format!("/// {k}")]
        {
            assert!(!holds(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_in_literals() {
        let k = Unsafe::KEYWORD;
        for code in [
            format!("const CSP: &str = \"script-src '{k}-inline'\";"),
            format!("let s = \"\\\" {k} {{\";"),
            format!("let s = b\"{k}\";"),
            format!("let s = r\"{k} {{\";"),
            format!("let s = r#\"a \" {k} {{ \"#;"),
            format!("let s = br##\"\"# {k}\"##;"),
            format!("let s = c\"{k}\";"),
        ] {
            assert!(!holds(&code), "{code}");
        }
    }

    #[test]
    fn the_lint_name_in_a_literal_is_not_code() {
        let code = format!("let s = \"{}\";", Unsafe::LINT);
        assert!(!holds(&code), "{code}");
    }

    #[test]
    fn unbalanced_text_is_refused() {
        assert!(Unsafe::holds("fn f( {").is_err());
    }
}
