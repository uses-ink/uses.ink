//! The catalog, parsed and checked. Every flaw in the file is reported at once.

use std::fmt;

use heck::ToUpperCamelCase;

/// A checked catalog: diagnostics and notices, in file order.
#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) diagnostics: Vec<Entry>,
    pub(crate) notices: Vec<Entry>,
}

#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) code: String,
    /// The Rust variant: `link.target.ambiguous` is `LinkTargetAmbiguous`.
    pub(crate) variant: String,
    /// Both set for a diagnostic, neither for a notice.
    pub(crate) severity: Option<Severity>,
    pub(crate) fault: Option<Fault>,
    pub(crate) fields: Vec<Field>,
    pub(crate) message: Template,
    pub(crate) fix: Option<Template>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    Author,
    Source,
    Operator,
    Ours,
}

#[derive(Debug)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) kind: Kind,
}

/// The closed set of field types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Path,
    Paths,
    Route,
    Text,
    Count,
    Bytes,
    Span,
    Decision,
}

impl Kind {
    const NAMES: &[(&str, Self)] = &[
        ("path", Self::Path),
        ("paths", Self::Paths),
        ("route", Self::Route),
        ("text", Self::Text),
        ("count", Self::Count),
        ("bytes", Self::Bytes),
        ("span", Self::Span),
        ("decision", Self::Decision),
    ];
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
    fn key(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Fix => "fix",
        }
    }
}

/// One thing wrong with the catalog.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Flaw {
    /// A top-level key other than `diagnostic` and `notice`, or one that is not a table.
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
        key: &'static str,
    },
    /// A key whose value is not a string.
    NotText {
        code: String,
        key: String,
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
                write!(out, "`{name}` is not a section; the sections are `diagnostic` and `notice`")
            }
            Self::Code { code } => write!(out, "`{code}` is not a code: write `noun.noun.reason` in kebab case"),
            Self::Collision { code, other } => write!(out, "`{code}` and `{other}` make the same Rust name"),
            Self::NotTable { code } => write!(out, "`{code}` is not a table"),
            Self::Key { code, key } => write!(out, "`{code}` has `{key}`, which it may not have"),
            Self::Missing { code, key } => write!(out, "`{code}` has no `{key}`"),
            Self::NotText { code, key } => write!(out, "`{code}`: `{key}` is not a string"),
            Self::Severity { code, given } => {
                write!(out, "`{code}`: `{given}` is not a severity; write error, warning or note")
            }
            Self::Fault { code, given } => {
                write!(out, "`{code}`: `{given}` is not a fault; write author, source, operator or ours")
            }
            Self::FieldName { code, field } => {
                write!(out, "`{code}`: `{field}` is not a field name; write a lower-case Rust identifier")
            }
            Self::FieldType { code, field, given } => {
                let known: Vec<&str> = Kind::NAMES.iter().map(|(name, _)| *name).collect();
                write!(out, "`{code}`: field `{field}` has type `{given}`; the types are {}", known.join(", "))
            }
            Self::Undeclared { code, part, field } => {
                write!(out, "`{code}`: the {} uses `{{{field}}}`, which is not a declared field", part.key())
            }
            Self::Brace { code, part, at } => {
                write!(out, "`{code}`: the {} has an unmatched brace at byte {at}; write `{{{{` or `}}}}`", part.key())
            }
        }
    }
}

impl Catalog {
    pub(crate) fn parse(table: toml::Table) -> Result<Self, Vec<Flaw>> {
        let mut flaws = Vec::new();
        let mut catalog = Self { diagnostics: Vec::new(), notices: Vec::new() };
        for (name, value) in table {
            let (toml::Value::Table(entries), notice) = (value, name == "notice") else {
                flaws.push(Flaw::Section { name });
                continue;
            };
            if !notice && name != "diagnostic" {
                flaws.push(Flaw::Section { name });
                continue;
            }
            for (code, value) in entries {
                if let Some(entry) = Entry::parse(code, value, notice, &mut flaws) {
                    if notice { catalog.notices.push(entry) } else { catalog.diagnostics.push(entry) }
                }
            }
        }
        catalog.collisions(&mut flaws);
        if flaws.is_empty() { Ok(catalog) } else { Err(flaws) }
    }

    fn collisions(&self, flaws: &mut Vec<Flaw>) {
        let all: Vec<&Entry> = self.diagnostics.iter().chain(&self.notices).collect();
        for (i, entry) in all.iter().enumerate() {
            if let Some(other) = all.iter().skip(i + 1).find(|o| o.variant == entry.variant) {
                flaws.push(Flaw::Collision { code: entry.code.clone(), other: other.code.clone() });
            }
        }
    }
}

impl Entry {
    fn parse(code: String, value: toml::Value, notice: bool, flaws: &mut Vec<Flaw>) -> Option<Self> {
        let before = flaws.len();
        if !is_code(&code) {
            flaws.push(Flaw::Code { code: code.clone() });
        }
        let toml::Value::Table(table) = value else {
            flaws.push(Flaw::NotTable { code });
            return None;
        };
        unknown_keys(&code, &table, notice, flaws);
        let text = |key: &'static str, flaws: &mut Vec<Flaw>| match table.get(key) {
            None => None,
            Some(toml::Value::String(text)) => Some(text.as_str()),
            Some(_) => {
                flaws.push(Flaw::NotText { code: code.clone(), key: key.to_owned() });
                None
            }
        };
        let needed = |key: &'static str, flaws: &mut Vec<Flaw>| {
            let found = text(key, flaws);
            if found.is_none() && table.get(key).is_none_or(toml::Value::is_str) {
                flaws.push(Flaw::Missing { code: code.clone(), key });
            }
            found
        };
        let (severity, fault) = if notice {
            (None, None)
        } else {
            let severity = needed("severity", flaws).and_then(|given| severity(&code, given, flaws));
            (severity, needed("fault", flaws).and_then(|given| fault(&code, given, flaws)))
        };
        let fields = fields(&code, table.get("fields"), flaws);
        let message = needed("message", flaws)
            .and_then(|t| template(Within { code: &code, part: Part::Message }, t, &fields, flaws));
        let fix = text("fix", flaws).and_then(|t| template(Within { code: &code, part: Part::Fix }, t, &fields, flaws));
        if flaws.len() > before {
            return None;
        }
        let variant = code.replace('.', "-").to_upper_camel_case();
        Some(Self { code, variant, severity, fault, fields, message: message?, fix })
    }
}

fn unknown_keys(code: &str, table: &toml::Table, notice: bool, flaws: &mut Vec<Flaw>) {
    let allowed: &[&str] =
        if notice { &["fields", "message", "fix"] } else { &["severity", "fault", "fields", "message", "fix"] };
    for key in table.keys().filter(|key| !allowed.contains(&key.as_str())) {
        flaws.push(Flaw::Key { code: code.to_owned(), key: key.clone() });
    }
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

fn severity(code: &str, given: &str, flaws: &mut Vec<Flaw>) -> Option<Severity> {
    let found = match given {
        "error" => Some(Severity::Error),
        "warning" => Some(Severity::Warning),
        "note" => Some(Severity::Note),
        _ => None,
    };
    if found.is_none() {
        flaws.push(Flaw::Severity { code: code.to_owned(), given: given.to_owned() });
    }
    found
}

fn fault(code: &str, given: &str, flaws: &mut Vec<Flaw>) -> Option<Fault> {
    let found = match given {
        "author" => Some(Fault::Author),
        "source" => Some(Fault::Source),
        "operator" => Some(Fault::Operator),
        "ours" => Some(Fault::Ours),
        _ => None,
    };
    if found.is_none() {
        flaws.push(Flaw::Fault { code: code.to_owned(), given: given.to_owned() });
    }
    found
}

fn fields(code: &str, value: Option<&toml::Value>, flaws: &mut Vec<Flaw>) -> Vec<Field> {
    let table = match value {
        None => return Vec::new(),
        Some(toml::Value::Table(table)) => table,
        Some(_) => {
            flaws.push(Flaw::NotText { code: code.to_owned(), key: "fields".to_owned() });
            return Vec::new();
        }
    };
    let mut fields = Vec::new();
    for (name, kind) in table {
        let named = name.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
            && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            && syn::parse_str::<syn::Ident>(name).is_ok();
        if !named {
            flaws.push(Flaw::FieldName { code: code.to_owned(), field: name.clone() });
        }
        let given = kind.as_str().map_or_else(|| kind.to_string(), str::to_owned);
        match Kind::NAMES.iter().find(|(n, _)| *n == given) {
            Some(&(_, kind)) if named => fields.push(Field { name: name.clone(), kind }),
            Some(_) => {}
            None => flaws.push(Flaw::FieldType { code: code.to_owned(), field: name.clone(), given }),
        }
    }
    fields
}

/// Which template of which entry is being read, as its flaws name it.
#[derive(Clone, Copy)]
struct Within<'a> {
    code: &'a str,
    part: Part,
}

/// `{name}` is a field, `{{` and `}}` are braces, anything else is text.
fn template(within: Within<'_>, text: &str, fields: &[Field], flaws: &mut Vec<Flaw>) -> Option<Template> {
    let Within { code, part } = within;
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut rest = text.char_indices().peekable();
    let brace = |at: usize| Flaw::Brace { code: code.to_owned(), part, at };
    while let Some((at, c)) = rest.next() {
        match c {
            '{' if rest.next_if(|&(_, c)| c == '{').is_some() => literal.push('{'),
            '}' if rest.next_if(|&(_, c)| c == '}').is_some() => literal.push('}'),
            '}' => {
                flaws.push(brace(at));
                return None;
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match rest.next() {
                        Some((_, '}')) => break,
                        Some((_, c)) if c != '{' => name.push(c),
                        _ => {
                            flaws.push(brace(at));
                            return None;
                        }
                    }
                }
                if !fields.iter().any(|f| f.name == name) {
                    flaws.push(Flaw::Undeclared { code: code.to_owned(), part, field: name.clone() });
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

#[cfg(test)]
mod test;
