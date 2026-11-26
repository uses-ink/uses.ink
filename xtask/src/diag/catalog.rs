//! The catalog, read and checked. Every flaw in the file is reported at once.

use std::fmt;

use heck::ToUpperCamelCase;

mod field;

pub(crate) use field::{Field, Kind};

use field::Docs;

/// A checked catalog: diagnostics and notices, in file order.
#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) diagnostics: Vec<Entry<Rank>>,
    pub(crate) notices: Vec<Entry<()>>,
}

/// One code of the catalog. `C` is what its class adds: a diagnostic's [`Rank`], nothing for a
/// notice.
#[derive(Debug)]
pub(crate) struct Entry<C> {
    pub(crate) code: String,
    /// The Rust variant: `link.target.ambiguous` is `LinkTargetAmbiguous`.
    pub(crate) variant: String,
    pub(crate) class: C,
    pub(crate) fields: Vec<Field>,
    pub(crate) message: Template,
    pub(crate) fix: Option<Template>,
}

/// What a diagnostic has that a notice does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rank {
    pub(crate) severity: Severity,
    pub(crate) fault: Fault,
}

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

/// A closed set the catalog spells as text: each member and its name.
pub(crate) trait Named: Copy + 'static {
    /// Every member, by name.
    const NAMES: &'static [(&'static str, Self)];

    /// The member named `name`.
    fn find(name: &str) -> Option<Self> {
        Self::NAMES.iter().find(|(n, _)| *n == name).map(|&(_, member)| member)
    }
}

/// A [`Named`] set an entry chooses from under one key, refused with a flaw of its own.
pub(crate) trait Choice: Named {
    /// The flaw for `given`, which names no member.
    fn refuse(code: String, given: String) -> Flaw;
}

/// The sections of the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Diagnostic,
    Notice,
}

impl Named for Section {
    const NAMES: &'static [(&'static str, Self)] = &[("diagnostic", Self::Diagnostic), ("notice", Self::Notice)];
}

impl Section {
    /// `[field]`, read before the others: each field name's doc.
    const FIELD: &'static str = "field";
}

/// The keys an entry may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    Severity,
    Fault,
    Fields,
    Message,
    Fix,
}

impl Key {
    const ALL: [Self; 5] = [Self::Severity, Self::Fault, Self::Fields, Self::Message, Self::Fix];

    /// The key named `name`.
    fn find(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == name)
    }

    /// Its name in the catalog.
    const fn name(self) -> &'static str {
        match self {
            Self::Severity => "severity",
            Self::Fault => "fault",
            Self::Fields => "fields",
            Self::Message => "message",
            Self::Fix => "fix",
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
    Note,
}

impl Named for Severity {
    const NAMES: &'static [(&'static str, Self)] =
        &[("error", Self::Error), ("warning", Self::Warning), ("note", Self::Note)];
}

impl Choice for Severity {
    fn refuse(code: String, given: String) -> Flaw {
        Flaw::Severity { code, given }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    Author,
    Source,
    Operator,
    Ours,
}

impl Named for Fault {
    const NAMES: &'static [(&'static str, Self)] =
        &[("author", Self::Author), ("source", Self::Source), ("operator", Self::Operator), ("ours", Self::Ours)];
}

impl Choice for Fault {
    fn refuse(code: String, given: String) -> Flaw {
        Flaw::Fault { code, given }
    }
}

/// A message or fix: literal text and fields, in order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Template(pub(crate) Vec<Piece>);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Text(String),
    Field(String),
}

impl Template {
    /// The fields it uses, each once, in first-use order.
    pub(crate) fn fields(&self) -> Vec<&str> {
        let mut used: Vec<&str> = Vec::new();
        for piece in &self.0 {
            if let Piece::Field(name) = piece
                && !used.contains(&name.as_str())
            {
                used.push(name);
            }
        }
        used
    }
}

/// Which template of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Message,
    Fix,
}

impl Part {
    /// The key it is written under.
    const fn key(self) -> Key {
        match self {
            Self::Message => Key::Message,
            Self::Fix => Key::Fix,
        }
    }
}

/// One thing wrong with the catalog.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Flaw {
    /// A top-level key other than `diagnostic`, `notice` and `field`, or one that is not a table.
    Section {
        name: String,
    },
    /// A code that is not `noun.noun.reason` in kebab case.
    Code {
        code: String,
    },
    /// Two codes that make the same Rust variant.
    Collision {
        code: String,
        other: String,
    },
    /// An entry that is not a table.
    NotTable {
        code: String,
    },
    /// A key the entry may not have.
    Key {
        code: String,
        key: String,
    },
    /// A key the entry needs.
    Missing {
        code: String,
        key: Key,
    },
    /// A key whose value is not a string, or not a table for `fields`.
    NotText {
        code: String,
        key: Key,
    },
    Severity {
        code: String,
        given: String,
    },
    Fault {
        code: String,
        given: String,
    },
    /// A field name that is not a lower-case Rust identifier.
    FieldName {
        code: String,
        field: String,
    },
    /// A field type outside the closed set.
    FieldType {
        code: String,
        field: String,
        given: String,
    },
    /// A doc in `[field]` that is not a string.
    FieldDoc {
        field: String,
    },
    /// A field whose name has no doc in `[field]`.
    Undocumented {
        code: String,
        field: String,
    },
    /// A doc in `[field]` for a name no entry has.
    Unused {
        field: String,
    },
    /// A template that uses a field the entry does not declare.
    Undeclared {
        code: String,
        part: Part,
        field: String,
    },
    /// A `{` or `}` that opens or closes nothing, at this byte offset of the template.
    Brace {
        code: String,
        part: Part,
        at: usize,
    },
}

impl fmt::Display for Flaw {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Section { name } => {
                write!(out, "`{name}` is not a section; the sections are `diagnostic`, `notice` and `field`")
            }
            Self::Code { code } => write!(out, "`{code}` is not a code: write `noun.noun.reason` in kebab case"),
            Self::Collision { code, other } => write!(out, "`{code}` and `{other}` make the same Rust name"),
            Self::NotTable { code } => write!(out, "`{code}` is not a table"),
            Self::Key { code, key } => write!(out, "`{code}` has `{key}`, which it may not have"),
            Self::Missing { code, key } => write!(out, "`{code}` has no `{key}`"),
            Self::NotText { code, key } => write!(out, "`{code}`: `{key}` is not a string"),
            Self::Severity { code, given } => {
                write!(out, "`{code}`: `{given}` is not a severity; write {}", Either::<Severity>::SET)
            }
            Self::Fault { code, given } => {
                write!(out, "`{code}`: `{given}` is not a fault; write {}", Either::<Fault>::SET)
            }
            Self::FieldName { code, field } => {
                write!(out, "`{code}`: `{field}` is not a field name; write a lower-case Rust identifier")
            }
            Self::FieldType { code, field, given } => {
                let known: Vec<&str> = Kind::NAMES.iter().map(|(name, _)| *name).collect();
                write!(out, "`{code}`: field `{field}` has type `{given}`; the types are {}", known.join(", "))
            }
            Self::FieldDoc { field } => write!(out, "`field.{field}` is not a string"),
            Self::Undocumented { code, field } => {
                write!(out, "`{code}`: field `{field}` has no doc; write one under `[field]`")
            }
            Self::Unused { field } => write!(out, "`field.{field}` documents a field no entry has"),
            Self::Undeclared { code, part, field } => {
                write!(out, "`{code}`: the {} uses `{{{field}}}`, which is not a declared field", part.key())
            }
            Self::Brace { code, part, at } => {
                write!(out, "`{code}`: the {} has an unmatched brace at byte {at}; write `{{{{` or `}}}}`", part.key())
            }
        }
    }
}

/// The members of a [`Named`] set as a choice reads: `a, b or c`.
struct Either<T>(std::marker::PhantomData<T>);

impl<T> Either<T> {
    const SET: Self = Self(std::marker::PhantomData);
}

impl<T: Named> fmt::Display for Either<T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last = T::NAMES.len().saturating_sub(1);
        T::NAMES.iter().enumerate().try_for_each(|(i, (name, _))| {
            let sep = match i {
                0 => "",
                i if i == last => " or ",
                _ => ", ",
            };
            write!(out, "{sep}{name}")
        })
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

#[cfg(test)]
mod test;
