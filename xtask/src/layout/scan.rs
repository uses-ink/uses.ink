//! Whether Rust source holds unsafe code: the keyword as a word of code, or the lint that guards
//! it, outside comments and string and char literals. A lexer just big enough for that, not a
//! parser: it knows comments (`//`, nested `/* */`), strings (`"..."`, `b"..."`, `c"..."`), raw
//! strings (`r"..."`, `r#"..."#`, `br#"..."#`), char literals and lifetimes.

/// The keyword, spelled in two parts so this file does not match itself.
pub(super) const KEYWORD: &str = concat!("un", "safe");

/// The lint that guards the keyword, which only a file allowed unsafe code may name.
const LINT: &str = concat!("un", "safe_code");

/// Whether `text` holds the keyword or names its lint as code.
pub(super) fn holds_unsafe(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        let next = chars.get(at + 1).copied();
        at = match (c, next) {
            ('/', Some('/')) => skip_line(&chars, at),
            ('/', Some('*')) => skip_block(&chars, at + 2),
            ('"', _) => skip_string(&chars, at + 1),
            ('\'', _) => skip_quote(&chars, at),
            (c, _) if is_ident(c) => {
                let end = chars.iter().skip(at).position(|&c| !is_ident(c)).map_or(chars.len(), |n| at + n);
                let word: String = chars.get(at..end).unwrap_or_default().iter().collect();
                match (word.as_str(), chars.get(end)) {
                    (KEYWORD | LINT, _) => return true,
                    ("r" | "br" | "cr", Some('"' | '#')) => skip_raw(&chars, end),
                    ("b" | "c", Some('"')) => skip_string(&chars, end + 1),
                    ("b", Some('\'')) => skip_quote(&chars, end),
                    _ => end,
                }
            }
            _ => at + 1,
        };
    }
    false
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Past the end of the line `at` is on.
fn skip_line(chars: &[char], at: usize) -> usize {
    chars.iter().skip(at).position(|&c| c == '\n').map_or(chars.len(), |n| at + n + 1)
}

/// Past the `*/` that closes a block comment whose body starts at `at`; block comments nest.
fn skip_block(chars: &[char], mut at: usize) -> usize {
    let mut depth = 1_usize;
    while let Some(&c) = chars.get(at) {
        match (c, chars.get(at + 1)) {
            ('/', Some('*')) => (depth, at) = (depth + 1, at + 2),
            ('*', Some('/')) if depth == 1 => return at + 2,
            ('*', Some('/')) => (depth, at) = (depth - 1, at + 2),
            _ => at += 1,
        }
    }
    at
}

/// Past the `"` that closes a string whose body starts at `at`, minding escapes.
fn skip_string(chars: &[char], mut at: usize) -> usize {
    while let Some(&c) = chars.get(at) {
        match c {
            '\\' => at += 2,
            '"' => return at + 1,
            _ => at += 1,
        }
    }
    at
}

/// Past a raw string whose `#`s or opening `"` start at `at`; a raw identifier (`r#name`) is
/// left for the caller to read as a word.
fn skip_raw(chars: &[char], at: usize) -> usize {
    let hashes = chars.iter().skip(at).take_while(|&&c| c == '#').count();
    if chars.get(at + hashes) != Some(&'"') {
        return at + hashes;
    }
    let mut end = at + hashes + 1;
    while let Some(&c) = chars.get(end) {
        let closed = chars.iter().skip(end + 1).take(hashes).filter(|&&c| c == '#').count() == hashes;
        if c == '"' && closed {
            return end + 1 + hashes;
        }
        end += 1;
    }
    end
}

/// Past a char literal at `at` (`'x'`, `'\n'`, `'\u{..}'`), or past the `'` of a lifetime.
fn skip_quote(chars: &[char], at: usize) -> usize {
    match (chars.get(at + 1), chars.get(at + 2)) {
        (Some('\\'), _) => chars.iter().skip(at + 3).position(|&c| c == '\'').map_or(chars.len(), |n| at + 3 + n + 1),
        (Some(_), Some('\'')) => at + 3,
        _ => at + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_in_every_spelling() {
        let k = KEYWORD;
        for code in [
            format!("{k} {{ x }}"),
            format!("{k}{{*p}}"),
            format!("{k}\n{{"),
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
            assert!(holds_unsafe(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_as_part_of_a_word() {
        let k = KEYWORD;
        for code in [format!("let {k}ty = 1;"), format!("fn is_{k}() {{}}"), format!("let r#{k}x = 1;")] {
            assert!(!holds_unsafe(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_in_comments() {
        let k = KEYWORD;
        for code in
            [format!("// {k} {{ }}"), format!("/* {k} {{ }} */"), format!("/* /* */ {k} */"), format!("/// {k}")]
        {
            assert!(!holds_unsafe(&code), "{code}");
        }
    }

    #[test]
    fn unsafe_in_literals() {
        let k = KEYWORD;
        for code in [
            format!("const CSP: &str = \"script-src '{k}-inline'\";"),
            format!("let s = \"\\\" {k} {{\";"),
            format!("let s = b\"{k}\";"),
            format!("let s = r\"{k} {{\";"),
            format!("let s = r#\"a \" {k} {{ \"#;"),
            format!("let s = br##\"\"# {k}\"##;"),
            format!("let s = c\"{k}\";"),
        ] {
            assert!(!holds_unsafe(&code), "{code}");
        }
    }

    #[test]
    fn the_lint_name_in_a_literal_is_not_code() {
        let code = format!("let s = \"{LINT}\";");
        assert!(!holds_unsafe(&code), "{code}");
    }
}
