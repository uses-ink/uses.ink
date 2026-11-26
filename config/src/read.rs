//! Reading `ink.toml` into a [`Config`]: a walk over toml's spanned tree, so every key and value
//! keeps where it was written. The only module that sees the `toml` crate.

use std::collections::BTreeMap;
use std::ops::Range;

use model::{ContentPath, DecisionCode, Diagnostic, Problem, Spanned};
use toml::de::{DeTable, DeValue};

use crate::place::Reach;
use crate::place::{self, KeyPath, across, span};
use crate::shape::Shape;
use crate::theme::{Theme, ThemeId, Token, is_kebab};
use crate::{Config, Parsed, Pattern, Pin};

mod value;

type Entry<'a, 'i> = (&'a toml::Spanned<toml::de::DeString<'i>>, &'a toml::Spanned<DeValue<'i>>);

pub(crate) fn parse(text: &str) -> Parsed {
    let (root, errors) = DeTable::parse_recoverable(text);
    if !errors.is_empty() {
        let diagnostics = errors
            .iter()
            .map(|error| {
                let problem = Problem::ConfigSyntaxInvalid { reason: error.message().to_owned() };
                let diagnostic = Diagnostic::new(problem);
                match error.span() {
                    Some(range) => diagnostic.at(span(range)),
                    None => diagnostic,
                }
            })
            .collect();
        return Parsed { config: Config::default(), diagnostics };
    }
    let mut reader = Reader::default();
    for entry in root.get_ref() {
        reader.top(entry);
    }
    Parsed { config: reader.config, diagnostics: reader.diagnostics }
}

#[derive(Default)]
struct Reader {
    config: Config,
    diagnostics: Vec<Diagnostic>,
}

impl Reader {
    fn unknown(&mut self, key: &KeyPath<'_>, at: Range<usize>) {
        self.diagnostics.push(Diagnostic::new(Problem::ConfigKeyUnknown { key: key.to_string() }).at(span(at)));
    }

    fn invalid(&mut self, key: &KeyPath<'_>, expected: &Shape<'_>, at: Range<usize>) {
        let problem = Problem::ConfigValueInvalid { key: key.to_string(), expected: expected.to_string() };
        self.diagnostics.push(Diagnostic::new(problem).at(span(at)));
    }

    /// The table of `value`, or a diagnostic saying `key` takes one.
    fn table<'a, 'i>(
        &mut self,
        key: &KeyPath<'_>,
        value: &'a toml::Spanned<DeValue<'i>>,
        expected: &Shape<'_>,
    ) -> Option<&'a DeTable<'i>> {
        let table = value.get_ref().as_table();
        if table.is_none() {
            self.invalid(key, expected, value.span());
        }
        table
    }

    fn top(&mut self, (key, value): Entry<'_, '_>) {
        let name = key.get_ref().as_ref();
        let path = KeyPath(vec![name]);
        match name {
            "theme" => self.theme(value),
            "include" => self.config.include = self.patterns(&path, value),
            "exclude" => self.config.exclude = self.patterns(&path, value),
            _ if place::holds(name, Reach::Site) || place::holds(name, Reach::File) => {
                let Some(table) = self.table(&path, value, &Shape::Table(name)) else { return };
                for entry in table {
                    self.pin_table(name, entry);
                }
            }
            _ => self.unknown(&path, key.span()),
        }
    }

    /// One entry of a pin table: a sitewide pin, or a file's table of pins.
    fn pin_table(&mut self, table: &str, (key, value): Entry<'_, '_>) {
        let name = key.get_ref().as_ref();
        let path = KeyPath(vec![table, name]);
        if let Some(code) = place::code_at(table, name).filter(|code| code.is_sitewide()) {
            if let Some(pin) = self.pin(&path, code, (key, value)) {
                self.config.site.insert(code, pin);
            }
            return;
        }
        if !place::holds(table, Reach::File) {
            return self.unknown(&path, key.span());
        }
        let file = match ContentPath::parse(name) {
            Ok(file) => file,
            Err(error) => return self.invalid(&path, &Shape::FileKey(error), key.span()),
        };
        let Some(pins) = self.table(&path, value, &Shape::FileTable { table, file: &file }) else { return };
        let mut read = BTreeMap::new();
        for (key, value) in pins {
            let name = key.get_ref().as_ref();
            let path = KeyPath(vec![table, file.as_str(), name]);
            match place::code_at(table, name).filter(|code| !code.is_sitewide()) {
                Some(code) => {
                    if let Some(pin) = self.pin(&path, code, (key, value)) {
                        read.insert(code, pin);
                    }
                }
                None => self.unknown(&path, key.span()),
            }
        }
        if !read.is_empty() {
            self.config.files.entry(file).or_default().extend(read);
        }
    }

    fn pin(&mut self, path: &KeyPath<'_>, code: DecisionCode, (key, value): Entry<'_, '_>) -> Option<Pin> {
        match value::read(code, value) {
            Ok(decided) => Some(Pin { decided, span: across(key.span(), value.span()) }),
            Err(at) => {
                self.invalid(path, &Shape::Pin(code), at);
                None
            }
        }
    }

    fn patterns(&mut self, path: &KeyPath<'_>, value: &toml::Spanned<DeValue<'_>>) -> Vec<Spanned<Pattern>> {
        let Some(array) = value.get_ref().as_array() else {
            self.invalid(path, &Shape::Patterns(None), value.span());
            return Vec::new();
        };
        let mut patterns = Vec::new();
        for element in array {
            match element.get_ref().as_str().map(str::parse::<Pattern>) {
                Some(Ok(pattern)) => patterns.push(Spanned::new(pattern, span(element.span()))),
                Some(Err(error)) => self.invalid(path, &Shape::Patterns(Some(error)), element.span()),
                None => self.invalid(path, &Shape::Patterns(None), element.span()),
            }
        }
        patterns
    }

    fn theme(&mut self, value: &toml::Spanned<DeValue<'_>>) {
        let path = KeyPath(vec!["theme"]);
        let Some(table) = self.table(&path, value, &Shape::Theme) else { return };
        let mut theme = Theme { plugin: None, tokens: Vec::new(), span: span(value.span()) };
        for (key, value) in table {
            let name = key.get_ref().as_ref();
            let path = KeyPath(vec!["theme", name]);
            let text = value.get_ref().as_str();
            if name == "use" {
                match text.map(str::parse::<ThemeId>) {
                    Some(Ok(id)) => theme.plugin = Some(Spanned::new(id, span(value.span()))),
                    Some(Err(error)) => self.invalid(&path, &Shape::ThemeId(Some(error)), value.span()),
                    None => self.invalid(&path, &Shape::ThemeId(None), value.span()),
                }
            } else if !is_kebab(name) {
                self.unknown(&path, key.span());
            } else if let Some(text) = text {
                theme.tokens.push(Token {
                    name: Spanned::new(name.to_owned(), span(key.span())),
                    value: Spanned::new(text.to_owned(), span(value.span())),
                });
            } else {
                self.invalid(&path, &Shape::Token, value.span());
            }
        }
        self.config.theme = Some(theme);
    }
}
