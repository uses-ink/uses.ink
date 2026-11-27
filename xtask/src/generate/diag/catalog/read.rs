//! Reads the catalog from its TOML table, every flaw in the file at once.

use heck::ToUpperCamelCase;

use super::field::Docs;
use super::schema::Section;
use super::{Catalog, Choice, Entry, Field, Flaw, Key, Kind, Named, Part, Piece, Rank, Template};

/// The class of an entry: its keys beyond those every entry may have, and how they are read.
pub(crate) trait Class: Sized {
    /// Every key an entry of this class may have.
    const KEYS: &'static [Key];

    /// Reads the class's own keys, each flaw into the reader.
    fn read(reader: &mut Reader<'_>) -> Option<Self>;
}

impl Class for Rank {
    const KEYS: &'static [Key] = &[Key::Severity, Key::Fault, Key::Fields, Key::Message, Key::Fix];

    fn read(reader: &mut Reader<'_>) -> Option<Self> {
        let severity = reader.choice(Key::Severity);
        let fault = reader.choice(Key::Fault);
        Some(Self { severity: severity?, fault: fault? })
    }
}

impl Class for () {
    const KEYS: &'static [Key] = &[Key::Fields, Key::Message, Key::Fix];

    fn read(_: &mut Reader<'_>) -> Option<Self> {
        Some(())
    }
}

impl TryFrom<toml::Table> for Catalog {
    type Error = Vec<Flaw>;

    fn try_from(mut table: toml::Table) -> Result<Self, Vec<Flaw>> {
        let mut flaws = Vec::new();
        let mut docs = match table.remove(Section::FIELD) {
            None => Docs::default(),
            Some(toml::Value::Table(section)) => Docs::read(section, &mut flaws),
            Some(_) => {
                flaws.push(Flaw::Section { name: Section::FIELD.to_owned() });
                Docs::default()
            }
        };
        let mut catalog = Self { diagnostics: Vec::new(), notices: Vec::new() };
        for (name, value) in table {
            let (toml::Value::Table(entries), Some(section)) = (value, Section::find(&name)) else {
                flaws.push(Flaw::Section { name });
                continue;
            };
            for (code, value) in entries {
                let mut reading = Reading { docs: &mut docs, flaws: &mut flaws };
                match section {
                    Section::Diagnostic => catalog.diagnostics.extend(Entry::read(code, value, &mut reading)),
                    Section::Notice => catalog.notices.extend(Entry::read(code, value, &mut reading)),
                }
            }
        }
        docs.unused(&mut flaws);
        catalog.collisions(&mut flaws);
        if flaws.is_empty() { Ok(catalog) } else { Err(flaws) }
    }
}

impl Catalog {
    /// Every pair of codes that make the same Rust variant.
    fn collisions(&self, flaws: &mut Vec<Flaw>) {
        let all: Vec<(&str, &str)> =
            self.diagnostics.iter().map(Entry::names).chain(self.notices.iter().map(Entry::names)).collect();
        for (i, (code, variant)) in all.iter().enumerate() {
            if let Some((other, _)) = all.iter().skip(i + 1).find(|(_, v)| v == variant) {
                flaws.push(Flaw::Collision { code: (*code).to_owned(), other: (*other).to_owned() });
            }
        }
    }
}

impl<C: Class> Entry<C> {
    /// Reads one entry, every flaw into the reading's flaws; `None` when it has any.
    fn read(code: String, value: toml::Value, reading: &mut Reading<'_>) -> Option<Self> {
        let Reading { docs, flaws } = reading;
        let before = flaws.len();
        if !Self::is_code(&code) {
            flaws.push(Flaw::Code { code: code.clone() });
        }
        let toml::Value::Table(table) = value else {
            flaws.push(Flaw::NotTable { code });
            return None;
        };
        let mut reader = Reader { code: &code, table: &table, docs, flaws };
        reader.unknown(C::KEYS);
        let class = C::read(&mut reader);
        let fields = reader.fields();
        let message = reader.needed(Key::Message).and_then(|t| reader.template(Part::Message, t, &fields));
        let fix = reader.text(Key::Fix).and_then(|t| reader.template(Part::Fix, t, &fields));
        if flaws.len() > before {
            return None;
        }
        let variant = code.replace('.', "-").to_upper_camel_case();
        Some(Self { code, variant, class: class?, fields, message: message?, fix })
    }

    /// Three words of lower-case letters and digits in kebab case, joined by `.`, each word
    /// starting with a letter.
    fn is_code(code: &str) -> bool {
        let word = |w: &str| {
            w.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
                && w.split('-')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
        };
        let words: Vec<&str> = code.split('.').collect();
        words.len() == 3 && words.iter().all(|w| word(w))
    }
}

impl<C> Entry<C> {
    /// Its code and its Rust variant.
    fn names(&self) -> (&str, &str) {
        (&self.code, &self.variant)
    }

    /// The template of `part`, if it has one.
    pub(crate) fn template(&self, part: Part) -> Option<&Template> {
        match part {
            Part::Message => Some(&self.message),
            Part::Fix => self.fix.as_ref(),
        }
    }
}

/// The catalog being read: the field docs entries take theirs from, and every flaw so far.
struct Reading<'a> {
    docs: &'a mut Docs,
    flaws: &'a mut Vec<Flaw>,
}

/// One entry's table being read: what it finds wrong goes into `flaws`, named by `code`.
pub(crate) struct Reader<'a> {
    code: &'a str,
    table: &'a toml::Table,
    docs: &'a mut Docs,
    flaws: &'a mut Vec<Flaw>,
}

impl<'a> Reader<'a> {
    /// Every key outside `allowed`.
    fn unknown(&mut self, allowed: &[Key]) {
        for key in self.table.keys().filter(|key| Key::find(key).is_none_or(|key| !allowed.contains(&key))) {
            self.flaws.push(Flaw::Key { code: self.code.to_owned(), key: key.clone() });
        }
    }

    /// The text under `key`, if it is there and is text.
    fn text(&mut self, key: Key) -> Option<&'a str> {
        match self.table.get(key.name()) {
            None => None,
            Some(toml::Value::String(text)) => Some(text.as_str()),
            Some(_) => {
                self.flaws.push(Flaw::NotText { code: self.code.to_owned(), key });
                None
            }
        }
    }

    /// The text under `key`, which the entry needs.
    fn needed(&mut self, key: Key) -> Option<&'a str> {
        let found = self.text(key);
        if found.is_none() && self.table.get(key.name()).is_none_or(toml::Value::is_str) {
            self.flaws.push(Flaw::Missing { code: self.code.to_owned(), key });
        }
        found
    }

    /// The member of `T` named under `key`, which the entry needs.
    fn choice<T: Choice>(&mut self, key: Key) -> Option<T> {
        let given = self.needed(key)?;
        let found = T::find(given);
        if found.is_none() {
            self.flaws.push(T::refuse(self.code.to_owned(), given.to_owned()));
        }
        found
    }

    /// The declared fields, in file order.
    fn fields(&mut self) -> Vec<Field> {
        let table = match self.table.get(Key::Fields.name()) {
            None => return Vec::new(),
            Some(toml::Value::Table(table)) => table,
            Some(_) => {
                self.flaws.push(Flaw::NotText { code: self.code.to_owned(), key: Key::Fields });
                return Vec::new();
            }
        };
        let mut fields = Vec::new();
        for (name, kind) in table {
            let named = name.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
                && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                && syn::parse_str::<syn::Ident>(name).is_ok();
            if !named {
                self.flaws.push(Flaw::FieldName { code: self.code.to_owned(), field: name.clone() });
            }
            let given = kind.as_str().map_or_else(|| kind.to_string(), str::to_owned);
            match Kind::find(&given) {
                Some(kind) if named => {
                    if let Some(doc) = self.docs.of(self.code, name, self.flaws) {
                        fields.push(Field { name: name.clone(), kind, doc });
                    }
                }
                Some(_) => {}
                None => self.flaws.push(Flaw::FieldType { code: self.code.to_owned(), field: name.clone(), given }),
            }
        }
        fields
    }

    /// The template `text` of `part`: `{name}` is a field, `{{` and `}}` are braces, anything
    /// else is text.
    fn template(&mut self, part: Part, text: &str, fields: &[Field]) -> Option<Template> {
        let code = self.code;
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut rest = text.char_indices().peekable();
        let brace = |at: usize| Flaw::Brace { code: code.to_owned(), part, at };
        while let Some((at, c)) = rest.next() {
            match c {
                '{' if rest.next_if(|&(_, c)| c == '{').is_some() => literal.push('{'),
                '}' if rest.next_if(|&(_, c)| c == '}').is_some() => literal.push('}'),
                '}' => {
                    self.flaws.push(brace(at));
                    return None;
                }
                '{' => {
                    let mut name = String::new();
                    loop {
                        match rest.next() {
                            Some((_, '}')) => break,
                            Some((_, c)) if c != '{' => name.push(c),
                            _ => {
                                self.flaws.push(brace(at));
                                return None;
                            }
                        }
                    }
                    if !fields.iter().any(|f| f.name == name) {
                        self.flaws.push(Flaw::Undeclared { code: code.to_owned(), part, field: name.clone() });
                    }
                    if !literal.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(&mut literal)));
                    }
                    pieces.push(Piece::Field(name));
                }
                c => literal.push(c),
            }
        }
        if !literal.is_empty() {
            pieces.push(Piece::Text(literal));
        }
        Some(Template(pieces))
    }
}
